//! `ShellPort` dla `AppCore` (okna, akcje plików jako użytkownik, ustawienia Windows) oraz
//! obsługa argumentów drugiej instancji / protokołu `alfa://` (lista dozwolonych w app-core).

use app_core::dto::{AlfaEvent, LocalizedText, SessionTemplate, ToastKind};
use app_core::ports::{ArtifactIntent, ArtifactIntentAction, ShellPort};
use app_core::protocol::{ProtocolAction, from_args};
use app_core::{AppCore, AppError};
use tauri::{AppHandle, Manager};

use crate::windows;

/// Powłoka Tauri.
pub struct TauriShell {
    app: AppHandle,
}

impl TauriShell {
    /// Powłoka na uchwycie aplikacji.
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

fn window_error(e: tauri::Error) -> AppError {
    AppError::internal(format!("okno: {e}"))
}

/// Uruchamia `explorer.exe` z argumentami (Windows); poza Windows — funkcja niedostępna.
fn explorer(args: &[&std::ffi::OsStr]) -> Result<(), AppError> {
    if cfg!(windows) {
        std::process::Command::new("explorer.exe")
            .args(args)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::internal(format!("explorer.exe: {e}")))
    } else {
        Err(AppError::unavailable("Akcja systemowa", "platform-windows"))
    }
}

impl ShellPort for TauriShell {
    fn show_main(&self, _session: Option<&str>) -> Result<(), AppError> {
        windows::show_main(&self.app).map_err(window_error)
    }

    fn hide_quick(&self) -> Result<(), AppError> {
        windows::hide_quick(&self.app).map_err(window_error)
    }

    fn open_system_settings(&self, uri: &str) -> Result<(), AppError> {
        explorer(&[uri.as_ref()])
    }

    fn artifact_action(&self, intent: &ArtifactIntent) -> Result<(), AppError> {
        match &intent.action {
            ArtifactIntentAction::Open => explorer(&[intent.path.as_os_str()]),
            ArtifactIntentAction::Reveal => {
                let mut select = std::ffi::OsString::from("/select,");
                select.push(intent.path.as_os_str());
                explorer(&[select.as_os_str()])
            }
            _ => Err(AppError::unavailable(
                "Ta akcja na pliku",
                "shell-integration",
            )),
        }
    }

    fn save_text_as(&self, _suggested_name: &str, _text: &str) -> Result<bool, AppError> {
        Err(AppError::unavailable(
            "Zapis przez okno dialogowe",
            "shell-integration",
        ))
    }
}

fn toast(core: &AppCore, pl: &str, en: &str) {
    core.emit_event(AlfaEvent::Toast {
        kind: ToastKind::Warning,
        message: LocalizedText::new(pl, en),
    });
}

/// Argumenty drugiej instancji (albo startowe): URI `alfa://…` z listy dozwolonych akcji.
pub fn handle_args(app: &AppHandle, argv: Vec<String>) {
    let core = app.state::<AppCore>().inner().clone();
    let app = app.clone();
    let action = from_args(argv.into_iter().skip(1));
    tauri::async_runtime::spawn(async move {
        let result = match action {
            None | Some(ProtocolAction::Open) => windows::show_main(&app).map_err(window_error),
            Some(ProtocolAction::QuickAsk) => windows::toggle_quick(&app).map_err(window_error),
            Some(ProtocolAction::OpenSession(id)) => {
                match core.app_set_active_session(Some(id)).await {
                    Ok(()) => windows::show_main(&app).map_err(window_error),
                    Err(e) => Err(e),
                }
            }
            Some(ProtocolAction::NewChat(text)) => match new_chat(&core, text).await {
                Ok(()) => windows::show_main(&app).map_err(window_error),
                Err(e) => Err(e),
            },
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "akcja protokołu alfa:// nie powiodła się");
            toast(&core, &e.message, "The alfa:// action failed.");
        }
    });
}

/// Nowa rozmowa (zasobnik, protokół): sesja + szkic (tekst z zewnątrz nie jest wysyłany).
pub async fn new_chat(core: &AppCore, text: Option<String>) -> Result<(), AppError> {
    let session = core.sessions_create(SessionTemplate::Empty).await?;
    if let Some(text) = text {
        core.sessions_save_draft(session.id.clone(), text).await?;
    }
    core.app_set_active_session(Some(session.id)).await
}
