//! Rdzeń narzędzi Office: ścieżki z deny-listą (przed Brokerem), zgody Brokera na zestaw
//! zdolności z weryfikacją każdego tokenu, wywołania portów poza wątkami asynchronicznymi,
//! mapowanie błędów Office/platformy na wyniki dla modelu, zdarzenia.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use compliance_contract::{DenyChecker, PathEnv};
use core_bus_contract::{EventBus, Level};
use platform_apps_contract::{OfficeApp, OfficeError, OfficePort};
use platform_contract::{FsPort, PlatformError};
use safety_broker_contract::{AppSelector, Capability, DeclaredFacts, PathScope};
use tools_common_contract::{
    Authorization, BrokerGate, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome,
    action_request, base_facts, paths, tool_event,
};
use tools_office_contract::OfficeToolsConfig;
use undo_journal_contract::UndoJournal;

/// Wynik pośredni: `Err` = gotowy wynik dla modelu.
pub(crate) type Step<T> = Result<T, Box<ToolOutcome>>;

/// Rdzeń współdzielony przez narzędzia.
pub(crate) struct Core {
    pub(crate) fs: Arc<dyn FsPort>,
    pub(crate) office: Arc<dyn OfficePort>,
    pub(crate) journal: Arc<dyn UndoJournal>,
    pub(crate) gate: BrokerGate,
    pub(crate) env: PathEnv,
    pub(crate) deny: DenyChecker,
    pub(crate) config: OfficeToolsConfig,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
}

pub(crate) fn invalid(text: impl std::fmt::Display) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(
        ToolErrorKind::InvalidArgs,
        format!("Niepoprawne argumenty: {text}."),
    ))
}

/// Błąd Office → wynik dla modelu.
pub(crate) fn office_failure(e: &OfficeError, action: &str) -> ToolOutcome {
    let (kind, hint) = match e {
        OfficeError::ProtectedView => {
            let mut o = ToolOutcome::denied(DenialReason::Policy, action);
            o.text = format!("Odmowa: {action} — {e}. Możesz tylko czytać ten plik (office_read).");
            return o;
        }
        OfficeError::Policy(_) => {
            let mut o = ToolOutcome::denied(DenialReason::Policy, action);
            o.text = format!("Odmowa: {action} — {e}. Popraw argumenty albo wybierz inną drogę.");
            return o;
        }
        OfficeError::NotInstalled(_) => (
            ToolErrorKind::Unsupported,
            "Office nie jest zainstalowany na tej maszynie.",
        ),
        OfficeError::Timeout { .. } => (
            ToolErrorKind::Timeout,
            "Office nie odpowiada — spróbuj później.",
        ),
        OfficeError::Document(_) => (
            ToolErrorKind::Io,
            "Dokument może być uszkodzony albo chroniony hasłem.",
        ),
        OfficeError::Unsupported(_) => (ToolErrorKind::Unsupported, ""),
    };
    ToolOutcome::failed(kind, format!("Nie wykonano: {action} — {e}. {hint}"))
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

impl Core {
    pub(crate) async fn emit(&self, name: &str, payload: serde_json::Value, ctx: &ToolCtx) {
        if let Some(bus) = &self.bus {
            let _ = bus
                .publish(tool_event(name, Level::Info, payload, ctx))
                .await;
        }
    }

    /// Ścieżka od modelu → ścieżka sprawdzona (postać, deny-lista — bez pytania Brokera).
    pub(crate) fn resolve(&self, raw: &str, ctx: &ToolCtx, action: &str) -> Step<String> {
        let path = paths::resolve_path(raw, ctx.workdir.as_deref(), &self.env)
            .map_err(|e| invalid(format!("ścieżka: {e}")))?;
        if self.deny.is_denied_path(&path, &self.env) || paths::has_credential_segment(&path) {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                &format!("{action} „{path}”"),
            )));
        }
        Ok(path)
    }

    pub(crate) fn scope(&self, path: &str) -> Step<PathScope> {
        paths::exact_scope(path, &self.env).map_err(|e| invalid(format!("ścieżka: {e}")))
    }

    /// `gui.control(winword.exe|excel.exe)`.
    pub(crate) fn app_capability(app: OfficeApp) -> Step<Capability> {
        AppSelector::parse(app.exe())
            .map(Capability::GuiControl)
            .map_err(|e| Box::new(ToolOutcome::failed(ToolErrorKind::Internal, e.to_string())))
    }

    /// Zgody Brokera na zdolności po kolei (odmowa → unieważnienie wydanych) i weryfikacja.
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
                let mut f: DeclaredFacts = base_facts(m, ctx);
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

    /// Odczyt pliku przez `FsPort` (wątek blokujący).
    pub(crate) async fn read_file(&self, path: &str, action: &str) -> Step<Vec<u8>> {
        let fs = self.fs.clone();
        let p = PathBuf::from(path);
        blocking(move || fs.read(&p))
            .await?
            .map_err(|e| Box::new(platform_failure(&e, action)))
    }

    /// Czy ścieżka istnieje.
    pub(crate) fn exists(&self, path: &str) -> bool {
        self.fs.exists(Path::new(path))
    }
}

/// Wywołanie blokujące (COM z limitem czasu, we/wy) poza wątkami asynchronicznymi.
pub(crate) async fn blocking<T, F>(work: F) -> Step<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(|e| {
        Box::new(ToolOutcome::failed(
            ToolErrorKind::Internal,
            format!("Wątek Office zakończył się błędem: {e}."),
        ))
    })
}
