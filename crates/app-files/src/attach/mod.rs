//! Załączniki composera (PLAN §11, §14.8): wybór plików (natywny dialog), przeciągnięcie (ścieżki
//! z systemowego zdarzenia upuszczenia w powłoce — UI ich nie podaje) i wklejenie ze schowka
//! systemowego (pliki albo obraz PNG). Każdy przyjęty plik jest **kopiowany** do katalogu sesji
//! `…\Sesje\<nazwa>\in` (podgląd w UI przez protokół zasobów, nie bajty przez IPC; treść tury
//! niezmienna po wysłaniu), z limitami liczby i rozmiaru. Przy wysłaniu kopie stają się
//! artefaktami sesji (`Origin::User`), a tura dostaje bloki `Attachment`.

mod check;
mod limits;
mod model;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use app_api::dto::{AttachmentInfo, AttachmentRejectReason, AttachmentRejection, AttachmentsAdded};
use app_api::error::AppError;
use app_api::paths::AppPaths;
use artifacts_contract::{Artifacts, Origin};
use platform_contract::{ClipboardContent, ClipboardPort};
use sessions_contract::{AttachmentRef, Block, SessionId, Sessions};

pub use check::{PathGuard, safe_name, unique_in};
pub use limits::{AttachmentLimits, Classified, LABEL_TOKENS, classify, image_magic_matches};
pub use model::{human_size, provider_blocks, turn_attachments};

/// Podkatalog sesji na kopie załączników.
pub const INBOX: &str = "in";
/// Jak długo czekamy na ścieżki z systemowego upuszczenia (zdarzenie UI może wyprzedzić powłokę).
const DROP_WAIT: Duration = Duration::from_millis(1500);
/// Po jakim czasie niepobrane upuszczenie wygasa.
const DROP_TTL: Duration = Duration::from_secs(15);

struct Staged {
    info: AttachmentInfo,
    path: PathBuf,
}

/// Załączniki przygotowane w composerach (w pamięci, per sesja).
pub struct Attachments {
    guard: PathGuard,
    sessions: Arc<dyn Sessions>,
    artifacts: Arc<dyn Artifacts>,
    clipboard: Arc<dyn ClipboardPort>,
    limits: AttachmentLimits,
    staged: Mutex<HashMap<SessionId, Vec<Staged>>>,
    drop: Mutex<Option<(Vec<PathBuf>, Instant)>>,
    next: Mutex<u64>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

impl Attachments {
    /// Załączniki nad sesjami, artefaktami i schowkiem.
    pub fn new(
        app: &AppPaths,
        sessions: Arc<dyn Sessions>,
        artifacts: Arc<dyn Artifacts>,
        clipboard: Arc<dyn ClipboardPort>,
        limits: AttachmentLimits,
    ) -> Self {
        Self {
            guard: PathGuard::new(app),
            sessions,
            artifacts,
            clipboard,
            limits,
            staged: Mutex::new(HashMap::new()),
            drop: Mutex::new(None),
            next: Mutex::new(0),
        }
    }

    /// Limity.
    pub fn limits(&self) -> &AttachmentLimits {
        &self.limits
    }

    /// Artefakty (projekcja historii dla modelu).
    pub fn artifacts(&self) -> &Arc<dyn Artifacts> {
        &self.artifacts
    }

    fn inbox(&self, session: &SessionId) -> Result<PathBuf, AppError> {
        let meta = self.sessions.session(session).map_err(AppError::from)?;
        Ok(meta.workdir.join(INBOX))
    }

    fn new_id(&self) -> String {
        let mut next = lock(&self.next);
        *next += 1;
        format!("att-{}-{}", std::process::id(), *next)
    }

    /// Przygotowane załączniki sesji (kolejność dodania).
    pub fn list(&self, session: &SessionId) -> Vec<AttachmentInfo> {
        lock(&self.staged)
            .get(session)
            .map(|v| v.iter().map(|s| s.info.clone()).collect())
            .unwrap_or_default()
    }

    /// Usuwa przygotowany załącznik (i jego kopię); nieznany identyfikator — bez zmian.
    pub fn remove(&self, session: &SessionId, id: &str) -> Vec<AttachmentInfo> {
        let removed = {
            let mut staged = lock(&self.staged);
            let list = staged.entry(session.clone()).or_default();
            let at = list.iter().position(|s| s.info.id == id);
            at.map(|i| list.remove(i))
        };
        if let Some(s) = removed {
            let _ = std::fs::remove_file(&s.path);
        }
        self.list(session)
    }

    /// Powłoka: ścieżki z systemowego upuszczenia na okno główne (jedno, ostatnie, z czasem życia).
    pub fn dropped(&self, paths: Vec<PathBuf>) {
        *lock(&self.drop) = Some((paths, Instant::now()));
    }

    /// Ścieżki ostatniego upuszczenia (pobierane raz; czeka chwilę, gdy zdarzenie UI wyprzedziło
    /// powłokę). Bez upuszczenia — pusto: UI nie może podać własnych ścieżek.
    pub async fn take_dropped(&self) -> Vec<PathBuf> {
        let deadline = Instant::now() + DROP_WAIT;
        loop {
            if let Some((paths, at)) = lock(&self.drop).take() {
                if at.elapsed() <= DROP_TTL {
                    return paths;
                }
                return Vec::new();
            }
            if Instant::now() >= deadline {
                return Vec::new();
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Dodaje pliki (kopie do katalogu sesji) z limitami; odrzucone z powodem.
    pub fn add_paths(
        &self,
        session: &SessionId,
        paths: &[PathBuf],
    ) -> Result<AttachmentsAdded, AppError> {
        let inbox = self.inbox(session)?;
        let mut out = AttachmentsAdded::default();
        for path in paths {
            let name = display_name(path);
            match self.admit(session, path, &inbox) {
                Ok(info) => out.added.push(info),
                Err(reason) => out.rejected.push(AttachmentRejection { name, reason }),
            }
        }
        out.staged = self.list(session);
        Ok(out)
    }

    /// Wklejenie ze schowka systemowego: lista plików albo obraz PNG; tekst — nic (UI wkleja sam).
    pub fn paste(&self, session: &SessionId) -> Result<AttachmentsAdded, AppError> {
        let content = self
            .clipboard
            .get()
            .map_err(|e| AppError::internal(format!("schowek: {e}")))?;
        match content {
            ClipboardContent::Files(paths) => self.add_paths(session, &paths),
            ClipboardContent::ImagePng(bytes) => self.add_png(session, &bytes),
            ClipboardContent::Text(_) | ClipboardContent::Empty => Ok(AttachmentsAdded {
                staged: self.list(session),
                ..AttachmentsAdded::default()
            }),
        }
    }

    /// Obraz PNG z pamięci (wklejony zrzut) jako plik `wklejony-obraz-….png`.
    pub fn add_png(&self, session: &SessionId, bytes: &[u8]) -> Result<AttachmentsAdded, AppError> {
        let inbox = self.inbox(session)?;
        let name = format!(
            "wklejony-obraz-{}.png",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        let mut out = AttachmentsAdded::default();
        let reject = |reason| AttachmentRejection {
            name: name.clone(),
            reason,
        };
        match self.room_for(session, bytes.len() as u64) {
            Err(reason) => out.rejected.push(reject(reason)),
            Ok(()) if !image_magic_matches("image/png", bytes) => {
                out.rejected
                    .push(reject(AttachmentRejectReason::Unreadable));
            }
            Ok(()) => {
                std::fs::create_dir_all(&inbox).map_err(storage)?;
                let dest = unique_in(&inbox, &name);
                std::fs::write(&dest, bytes).map_err(storage)?;
                match self.stage(session, &dest) {
                    Ok(info) => out.added.push(info),
                    Err(reason) => out.rejected.push(reject(reason)),
                }
            }
        }
        out.staged = self.list(session);
        Ok(out)
    }

    /// Miejsce na kolejny plik o rozmiarze `bytes` (limity liczby, pliku i sumy).
    fn room_for(&self, session: &SessionId, bytes: u64) -> Result<(), AttachmentRejectReason> {
        let staged = lock(&self.staged);
        let list = staged.get(session).map(Vec::as_slice).unwrap_or_default();
        if bytes == 0 {
            return Err(AttachmentRejectReason::Empty);
        }
        if bytes > self.limits.max_file_bytes {
            return Err(AttachmentRejectReason::TooLarge);
        }
        if list.len() >= self.limits.max_files {
            return Err(AttachmentRejectReason::TooMany);
        }
        let total: u64 = list.iter().map(|s| s.info.bytes).sum();
        if total + bytes > self.limits.max_total_bytes {
            return Err(AttachmentRejectReason::TotalTooLarge);
        }
        Ok(())
    }

    fn admit(
        &self,
        session: &SessionId,
        path: &Path,
        inbox: &Path,
    ) -> Result<AttachmentInfo, AttachmentRejectReason> {
        let size = self.guard.check(path)?;
        self.room_for(session, size)?;
        std::fs::create_dir_all(inbox).map_err(|_| AttachmentRejectReason::Unreadable)?;
        let dest = unique_in(inbox, &safe_name(&display_name(path)));
        if let Err(reason) = copy_capped(path, &dest, self.limits.max_file_bytes) {
            let _ = std::fs::remove_file(&dest);
            return Err(reason);
        }
        self.stage(session, &dest)
    }

    /// Klasyfikuje kopię i dopisuje ją do przygotowanych.
    fn stage(
        &self,
        session: &SessionId,
        copy: &Path,
    ) -> Result<AttachmentInfo, AttachmentRejectReason> {
        let bytes = std::fs::read(copy).map_err(|_| AttachmentRejectReason::Unreadable)?;
        let mime = artifacts_contract::guess_mime(copy).to_owned();
        let head = &bytes[..bytes.len().min(artifacts_contract::BINARY_SNIFF_BYTES)];
        let chars = (!artifacts_contract::looks_binary(head))
            .then(|| String::from_utf8_lossy(&bytes).chars().count());
        let c = classify(&mime, head, bytes.len() as u64, chars, &self.limits);
        let info = AttachmentInfo {
            id: self.new_id(),
            session_id: session.to_string(),
            name: display_name(copy),
            bytes: bytes.len() as u64,
            mime,
            kind: c.kind,
            path: copy.to_string_lossy().into_owned(),
            tokens: c.tokens,
            delivery: c.delivery,
        };
        lock(&self.staged)
            .entry(session.clone())
            .or_default()
            .push(Staged {
                info: info.clone(),
                path: copy.to_path_buf(),
            });
        Ok(info)
    }

    /// Przed wysłaniem: rejestruje wskazane załączniki jako artefakty sesji i zwraca bloki tury
    /// (przygotowane zostają do [`Self::commit`] — nieudane wysłanie ich nie gubi).
    pub fn prepare(&self, session: &SessionId, ids: &[String]) -> Result<Vec<Block>, AppError> {
        let chosen: Vec<(AttachmentInfo, PathBuf)> = {
            let staged = lock(&self.staged);
            let list = staged.get(session).map(Vec::as_slice).unwrap_or_default();
            ids.iter()
                .map(|id| {
                    list.iter()
                        .find(|s| &s.info.id == id)
                        .map(|s| (s.info.clone(), s.path.clone()))
                        .ok_or_else(|| AppError::invalid(format!("Nieznany załącznik „{id}”.")))
                })
                .collect::<Result<_, _>>()?
        };
        chosen
            .into_iter()
            .map(|(info, path)| {
                let artifact = self
                    .artifacts
                    .register(session, &path, Origin::User, None)
                    .map_err(|e| AppError::storage(format!("załącznik {}: {e}", info.name)))?;
                let latest = artifact.latest();
                Ok(Block::Attachment {
                    attachment: AttachmentRef {
                        name: info.name,
                        mime: info.mime,
                        artifact_id: Some(artifact.id.to_string()),
                        sha256: latest.map(|v| v.sha256.clone()),
                        bytes: latest.map(|v| v.bytes),
                    },
                })
            })
            .collect()
    }

    /// Po zapisaniu tury: wysłane załączniki znikają z composera.
    pub fn commit(&self, session: &SessionId, ids: &[String]) {
        if let Some(list) = lock(&self.staged).get_mut(session) {
            list.retain(|s| !ids.contains(&s.info.id));
        }
    }
}

fn storage(e: std::io::Error) -> AppError {
    AppError::storage(format!("załącznik: {e}"))
}

/// Kopiuje plik przez jeden uchwyt (rozmiar sprawdzany na otwartym pliku, najwyżej `max` bajtów).
fn copy_capped(from: &Path, to: &Path, max: u64) -> Result<(), AttachmentRejectReason> {
    let unreadable = |_| AttachmentRejectReason::Unreadable;
    let src = std::fs::File::open(from).map_err(unreadable)?;
    let meta = src.metadata().map_err(unreadable)?;
    if !meta.is_file() {
        return Err(AttachmentRejectReason::NotAFile);
    }
    if meta.len() > max {
        return Err(AttachmentRejectReason::TooLarge);
    }
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(unreadable)?;
    let copied = std::io::copy(&mut src.take(max + 1), &mut out).map_err(unreadable)?;
    if copied > max {
        return Err(AttachmentRejectReason::TooLarge);
    }
    if copied == 0 {
        return Err(AttachmentRejectReason::Empty);
    }
    out.flush().map_err(unreadable)?;
    Ok(())
}
