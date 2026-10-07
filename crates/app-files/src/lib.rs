//! Pliki w aplikacji (`app-*`, crates/README.md; PLAN §11, §14.8, §15.1):
//!
//! - [`attach`] — załączniki composera: wybór (natywny dialog), przeciągnięcie (ścieżki tylko
//!   z systemowego upuszczenia w powłoce), wklejenie ze schowka systemowego; kopie w katalogu
//!   sesji, limity, szacunek tokenów; przy wysłaniu — artefakty sesji i bloki tury; projekcja
//!   historii dla modelu (obraz base64, tekst jako treść niezaufana),
//! - [`export`] — eksport rozmowy (aktywna gałąź albo jedna wiadomość) do Markdown / HTML,
//! - [`backup`] — kopie zapasowe `.alfa` z harmonogramem, rotacją i testem przywracania,
//! - [`docs`] — artefakty sesji jako kategoria paczek `.alfa`.
//!
//! [`FilesApp`] spina je dla komend `attachments_*`, `sessions_export_conversation`, `backups_*`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod attach;
pub mod backup;
pub mod docs;
pub mod export;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use accounts_hub_contract::SecretStore;
use app_api::dto::{
    AttachmentInfo, AttachmentsAdded, BackupCheck, BackupConfig, BackupView, ConversationFormat,
    ExportResult, SecretInput,
};
use app_api::error::AppError;
use app_api::events::EventHub;
use app_api::ids;
use app_api::paths::AppPaths;
use app_api::ports::ShellPort;
use artifacts_contract::Artifacts;
use platform_contract::{ClipboardPort, SystemSignalsPort};
use providers_contract::ContentBlock;
use sessions_contract::{Block, SessionId, Sessions};
use transfer_contract::Transfer;

pub use attach::{AttachmentLimits, Attachments, PathGuard};
pub use backup::{BackupDeps, BackupService};
pub use docs::ArtifactDocuments;

/// Zależności złożenia.
pub struct FilesDeps {
    /// Katalogi aplikacji.
    pub paths: AppPaths,
    /// Sesje (katalog sesji, historia do eksportu, skażenie).
    pub sessions: Arc<dyn Sessions>,
    /// Rejestr artefaktów (załączniki wysłane).
    pub artifacts: Arc<dyn Artifacts>,
    /// Powłoka (dialogi).
    pub shell: Arc<dyn ShellPort>,
    /// Schowek (wklejanie plików i obrazów); `None` — schowek systemowy Windows.
    pub clipboard: Option<Arc<dyn ClipboardPort>>,
    /// Moduł `transfer` (kopie zapasowe).
    pub transfer: Option<Arc<dyn Transfer>>,
    /// Sekrety (hasło kopii).
    pub secrets: Option<Arc<dyn SecretStore>>,
    /// Sygnały systemowe (harmonogram kopii).
    pub signals: Option<Arc<dyn SystemSignalsPort>>,
    /// Zdarzenia UI.
    pub events: Option<EventHub>,
    /// Artefakty w paczkach `.alfa` (uzgadniane po imporcie przy otwarciu sesji).
    pub docs: Option<Arc<ArtifactDocuments>>,
}

/// Pliki w aplikacji.
pub struct FilesApp {
    attachments: Attachments,
    backups: Arc<BackupService>,
    sessions: Arc<dyn Sessions>,
    shell: Arc<dyn ShellPort>,
    docs: Option<Arc<ArtifactDocuments>>,
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::internal(format!("zadanie plików: {e}")))?
}

impl FilesApp {
    /// Składa część (bez uruchamiania harmonogramu — [`Self::start`]).
    pub fn open(d: FilesDeps) -> Arc<Self> {
        let guard = Arc::new(PathGuard::new(&d.paths));
        let backups = BackupService::new(BackupDeps {
            transfer: d.transfer,
            secrets: d.secrets,
            shell: d.shell.clone(),
            signals: d.signals,
            events: d.events,
            state_file: d.paths.state().join("backup.json"),
            guard,
        });
        Arc::new(Self {
            attachments: Attachments::new(
                &d.paths,
                d.sessions.clone(),
                d.artifacts,
                d.clipboard
                    .unwrap_or_else(|| Arc::new(platform_windows_impl::WindowsPlatform::default())),
                AttachmentLimits::default(),
            ),
            backups,
            sessions: d.sessions,
            shell: d.shell,
            docs: d.docs,
        })
    }

    /// Harmonogram kopii w tle (pierwsze sprawdzenie po 2 min, potem co 10 min).
    pub fn start(&self) {
        self.backups
            .spawn(Duration::from_secs(120), Duration::from_secs(600));
    }

    /// Kopie zapasowe.
    pub fn backups(&self) -> &Arc<BackupService> {
        &self.backups
    }

    /// Załączniki.
    pub fn attachments(&self) -> &Attachments {
        &self.attachments
    }

    fn session(&self, id: &str) -> Result<SessionId, AppError> {
        let id = ids::session(id)?;
        let meta = self.sessions.session(&id).map_err(AppError::from)?;
        if meta.trashed {
            return Err(AppError::not_found(format!("Sesja {id} jest w koszu.")));
        }
        Ok(id)
    }

    /// `attachments_pick`: natywny dialog wyboru plików.
    pub async fn attachments_pick(
        self: &Arc<Self>,
        session_id: &str,
    ) -> Result<AttachmentsAdded, AppError> {
        let session = self.session(session_id)?;
        let me = self.clone();
        blocking(move || {
            let paths = me.shell.pick_files()?;
            me.attachments.add_paths(&session, &paths)
        })
        .await
    }

    /// `attachments_add_dropped`: pliki z ostatniego systemowego upuszczenia na okno główne.
    pub async fn attachments_add_dropped(
        self: &Arc<Self>,
        session_id: &str,
    ) -> Result<AttachmentsAdded, AppError> {
        let session = self.session(session_id)?;
        let paths = self.attachments.take_dropped().await;
        let me = self.clone();
        blocking(move || me.attachments.add_paths(&session, &paths)).await
    }

    /// `attachments_paste`: pliki albo obraz ze schowka systemowego.
    pub async fn attachments_paste(
        self: &Arc<Self>,
        session_id: &str,
    ) -> Result<AttachmentsAdded, AppError> {
        let session = self.session(session_id)?;
        let me = self.clone();
        blocking(move || me.attachments.paste(&session)).await
    }

    /// `attachments_list` (otwarcie sesji w composerze — także uzgodnienie artefaktów z importu).
    pub fn attachments_list(&self, session_id: &str) -> Result<Vec<AttachmentInfo>, AppError> {
        let session = self.session(session_id)?;
        if let Some(docs) = &self.docs {
            docs.reconcile();
        }
        Ok(self.attachments.list(&session))
    }

    /// `attachments_remove`.
    pub fn attachments_remove(
        &self,
        session_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<AttachmentInfo>, AppError> {
        Ok(self
            .attachments
            .remove(&self.session(session_id)?, attachment_id))
    }

    /// Powłoka: ścieżki upuszczone na okno główne (pobierze je `attachments_add_dropped`).
    pub fn dropped(&self, paths: Vec<PathBuf>) {
        self.attachments.dropped(paths);
    }

    /// Wysyłanie tury: bloki załączników (artefakty sesji). Tekst i dokumenty z zewnątrz to treść
    /// niezaufana — sesja zostaje oznaczona jako skażona (flaga tylko rośnie).
    pub fn prepare(&self, session: &SessionId, ids: &[String]) -> Result<Vec<Block>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let blocks = self.attachments.prepare(session, ids)?;
        let external = attach::turn_attachments(&blocks)
            .iter()
            .any(|a| a.kind != app_api::dto::AttachmentKind::Image);
        if external {
            self.sessions
                .mark_tainted(session)
                .map_err(AppError::from)?;
        }
        Ok(blocks)
    }

    /// Po zapisaniu tury.
    pub fn commit(&self, session: &SessionId, ids: &[String]) {
        self.attachments.commit(session, ids);
    }

    /// Projekcja załączników tury dla modelu.
    pub fn provider_blocks(&self, session: &SessionId, blocks: &[Block]) -> Vec<ContentBlock> {
        attach::provider_blocks(
            self.attachments.artifacts().as_ref(),
            session,
            blocks,
            self.attachments.limits(),
        )
    }

    /// `sessions_export_conversation`: aktywna gałąź (albo jedna wiadomość) → natywny dialog
    /// zapisu → plik `.md` / `.html`.
    pub async fn export_conversation(
        self: &Arc<Self>,
        session_id: &str,
        format: ConversationFormat,
        turn_id: Option<String>,
    ) -> Result<ExportResult, AppError> {
        let session = self.session(session_id)?;
        let me = self.clone();
        blocking(move || me.export_blocking(&session, format, turn_id.as_deref())).await
    }

    fn export_blocking(
        &self,
        session: &SessionId,
        format: ConversationFormat,
        turn_id: Option<&str>,
    ) -> Result<ExportResult, AppError> {
        let meta = self.sessions.session(session).map_err(AppError::from)?;
        let turns = match turn_id {
            Some(t) => {
                let (owner, turn) = ids::parse_turn(t)?;
                if &owner != session {
                    return Err(AppError::invalid("Wiadomość z innej sesji."));
                }
                vec![self.sessions.turn(session, turn).map_err(AppError::from)?]
            }
            None => match self.sessions.active_leaf(session).map_err(AppError::from)? {
                Some(leaf) => self
                    .sessions
                    .branch_projection(session, leaf)
                    .map_err(AppError::from)?,
                None => Vec::new(),
            },
        };
        let messages = export::messages(&turns);
        if messages.is_empty() {
            return Err(AppError::invalid("Brak wiadomości do eksportu."));
        }
        let now = chrono::Utc::now();
        let (text, ext, filter) = match format {
            ConversationFormat::Markdown => (
                export::markdown(&meta.title, &messages, now),
                "md",
                "Markdown",
            ),
            ConversationFormat::Html => (export::html(&meta.title, &messages, now), "html", "HTML"),
        };
        let name = export::file_name(&meta.title, now, ext);
        let Some(path) = self.shell.pick_save_file(&name, filter, &[ext])? else {
            return Ok(ExportResult::Cancelled);
        };
        std::fs::write(&path, text.as_bytes())
            .map_err(|e| AppError::storage(format!("{}: {e}", path.display())))?;
        Ok(ExportResult::Saved {
            path: path.to_string_lossy().into_owned(),
            files: 1,
            bytes: text.len() as u64,
        })
    }

    /// `backups_status`.
    pub fn backups_status(&self) -> BackupView {
        self.backups.view()
    }

    /// `backups_configure`.
    pub fn backups_configure(&self, config: BackupConfig) -> Result<BackupView, AppError> {
        self.backups.configure(config)
    }

    /// `backups_choose_dir`.
    pub async fn backups_choose_dir(&self) -> Result<BackupView, AppError> {
        self.backups.choose_dir().await
    }

    /// `backups_set_password`.
    pub fn backups_set_password(
        &self,
        password: Option<SecretInput>,
    ) -> Result<BackupView, AppError> {
        self.backups.set_password(password)
    }

    /// `backups_run_now`.
    pub async fn backups_run_now(&self) -> Result<BackupView, AppError> {
        self.backups.run_now().await
    }

    /// `backups_verify`.
    pub async fn backups_verify(&self, file: &str) -> Result<BackupCheck, AppError> {
        self.backups.verify(file).await
    }
}
