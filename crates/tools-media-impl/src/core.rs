//! Rdzeń narzędzi multimediów: ścieżki z deny-listą (przed Brokerem, także po dowiązaniach),
//! zgody Brokera z weryfikacją każdego tokenu, nagłówki z `lib-media`, wywołania blokujące,
//! mapowanie błędów na wyniki dla modelu, zdarzenia.

use std::path::Path;
use std::sync::Arc;

use compliance_contract::{DenyChecker, PathEnv};
use core_bus_contract::{EventBus, Level};
use lib_media::{FileSource, Limits, MediaInfo, RangeRead, probe};
use platform_contract::{FsPort, PlatformError};
use safety_broker_contract::Capability;
use tools_common_contract::{
    Authorization, BrokerGate, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome,
    action_request, base_facts, paths, tool_event,
};
use tools_media_contract::{AudioPlayer, ConvertError, MediaToolsConfig, Transcoder};
use undo_journal_contract::UndoJournal;

/// Wynik pośredni: `Err` = gotowy wynik dla modelu.
pub(crate) type Step<T> = Result<T, Box<ToolOutcome>>;

/// Rdzeń współdzielony przez narzędzia.
pub(crate) struct Core {
    pub(crate) fs: Arc<dyn FsPort>,
    pub(crate) files: Arc<dyn RangeRead>,
    pub(crate) journal: Arc<dyn UndoJournal>,
    pub(crate) transcoder: Arc<dyn Transcoder>,
    pub(crate) player: Arc<dyn AudioPlayer>,
    pub(crate) gate: BrokerGate,
    pub(crate) env: PathEnv,
    pub(crate) deny: DenyChecker,
    pub(crate) config: MediaToolsConfig,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
}

pub(crate) fn invalid(text: impl std::fmt::Display) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(
        ToolErrorKind::InvalidArgs,
        format!("Niepoprawne argumenty: {text}."),
    ))
}

/// Błąd platformy → wynik dla modelu.
pub(crate) fn platform_failure(e: &PlatformError, action: &str) -> ToolOutcome {
    let kind = match e {
        PlatformError::Denylisted(_) => return ToolOutcome::denied(DenialReason::DenyList, action),
        PlatformError::NotFound(_) => ToolErrorKind::NotFound,
        PlatformError::AlreadyExists(_) => ToolErrorKind::AlreadyExists,
        PlatformError::InvalidPath(_) => ToolErrorKind::InvalidArgs,
        PlatformError::Unsupported(_) => ToolErrorKind::Unsupported,
        _ => ToolErrorKind::Io,
    };
    ToolOutcome::failed(kind, format!("Nie wykonano: {action} — {e}."))
}

/// Błąd konwersji → wynik dla modelu.
pub(crate) fn convert_failure(e: &ConvertError, action: &str) -> ToolOutcome {
    let (kind, hint) = match e {
        ConvertError::NotInstalled(_) => (
            ToolErrorKind::Unsupported,
            " Poproś właściciela o instalację ffmpeg (Ustawienia → Modele i silniki → ffmpeg).",
        ),
        ConvertError::Unsupported(_) => (ToolErrorKind::Unsupported, ""),
        ConvertError::TooLarge(_) => (
            ToolErrorKind::Io,
            " Wybierz krótszy fragment (`duration_s`) albo mniejszy rozmiar (`max_side`).",
        ),
        ConvertError::Timeout => (ToolErrorKind::Timeout, " Wybierz krótszy fragment."),
        ConvertError::Cancelled => return ToolOutcome::cancelled(action),
        ConvertError::Failed(_) => (ToolErrorKind::Io, ""),
    };
    ToolOutcome::failed(kind, format!("Nie wykonano: {action} — {e}.{hint}"))
}

impl Core {
    pub(crate) async fn emit(&self, name: &str, payload: serde_json::Value, ctx: &ToolCtx) {
        if let Some(bus) = &self.bus {
            let _ = bus
                .publish(tool_event(name, Level::Info, payload, ctx))
                .await;
        }
    }

    /// Ścieżka od modelu → ścieżka sprawdzona (postać, deny-lista — także po dowiązaniach).
    pub(crate) fn resolve(&self, raw: &str, ctx: &ToolCtx, action: &str) -> Step<String> {
        let path = paths::resolve_path(raw, ctx.workdir.as_deref(), &self.env)
            .map_err(|e| invalid(format!("ścieżka: {e}")))?;
        let denied = paths::protected_with_links(&path, |p| {
            self.deny.is_denied_path(p, &self.env) || paths::has_credential_segment(p)
        });
        if denied {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                &format!("{action} „{path}”"),
            )));
        }
        Ok(path)
    }

    /// `fs.read(plik)` albo `fs.write(plik)` dokładnie dla tej ścieżki.
    pub(crate) fn capability(&self, path: &str, write: bool) -> Step<Capability> {
        let scope =
            paths::exact_scope(path, &self.env).map_err(|e| invalid(format!("ścieżka: {e}")))?;
        Ok(if write {
            Capability::FsWrite(scope)
        } else {
            Capability::FsRead(scope)
        })
    }

    /// Zgody Brokera na zdolności (odmowa → unieważnienie wydanych) i weryfikacja każdej.
    pub(crate) async fn authorize(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        caps: Vec<Capability>,
        action: &str,
    ) -> Step<Vec<Authorization>> {
        let requests = caps
            .iter()
            .map(|c| {
                let mut f = base_facts(m, ctx);
                f.touches_private_data = true;
                action_request(ctx, c.clone(), f)
            })
            .collect();
        let auths = self
            .gate
            .authorize_all(requests, ctx)
            .await
            .map_err(|e| Box::new(e.into_outcome(action)))?;
        for (auth, cap) in auths.iter().zip(&caps) {
            if let Err(e) = self.gate.verify(auth, cap, &ctx.holder) {
                self.gate.release(&auths).await;
                return Err(Box::new(e.into_outcome(action)));
            }
        }
        Ok(auths)
    }

    /// Nagłówki pliku (odczyt fragmentami na wątku blokującym).
    pub(crate) async fn probe(&self, path: &str, action: &str) -> Step<MediaInfo> {
        let files = self.files.clone();
        let p = path.to_owned();
        let r = blocking(move || {
            let mut src = FileSource::open(&*files, Path::new(&p)).map_err(Err)?;
            probe(&mut src, &Limits::default()).map_err(Ok)
        })
        .await?;
        r.map_err(|e| match e {
            Err(platform) => Box::new(platform_failure(&platform, action)),
            Ok(media) => Box::new(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Nie wykonano: {action} „{path}” — {media}."),
            )),
        })
    }
}

/// Wywołanie blokujące (ffmpeg, odczyt plików) poza wątkami asynchronicznymi.
pub(crate) async fn blocking<T, F>(work: F) -> Step<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(|e| {
        Box::new(ToolOutcome::failed(
            ToolErrorKind::Internal,
            format!("Wątek narzędzia zakończył się błędem: {e}."),
        ))
    })
}
