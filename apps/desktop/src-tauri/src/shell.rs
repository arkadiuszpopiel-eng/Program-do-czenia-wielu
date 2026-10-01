//! `ShellPort` dla `AppCore` (okna, natywne dialogi „Zapisz jako"/„Otwórz", akcje plików jako
//! użytkownik, ustawienia Windows) oraz obsługa argumentów drugiej instancji / protokołu `alfa://`
//! (lista dozwolonych w app-core; „przejdź do sesji" = zdarzenie `OpenSession` dla UI).

use std::path::PathBuf;

use app_core::dto::{AlfaEvent, LocalizedText, SessionTemplate, ToastKind};
use app_core::ports::{ArtifactIntent, ArtifactIntentAction, ShellPort};
use app_core::protocol::{ProtocolAction, from_args};
use app_core::{AppCore, AppError};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, FilePath};

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

/// Ścieżka z dialogu (na pulpicie zawsze ścieżka pliku, nie URI).
fn picked(path: Option<FilePath>) -> Result<Option<PathBuf>, AppError> {
    path.map(|p| {
        p.into_path()
            .map_err(|e| AppError::internal(format!("okno wyboru pliku: {e}")))
    })
    .transpose()
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

    fn save_text_as(&self, suggested_name: &str, text: &str) -> Result<bool, AppError> {
        let dialog = self.app.dialog().file().set_file_name(suggested_name);
        let Some(path) = picked(dialog.blocking_save_file())? else {
            return Ok(false);
        };
        std::fs::write(&path, text)
            .map(|()| true)
            .map_err(|e| AppError::storage(format!("{}: {e}", path.display())))
    }

    fn pick_save_path(&self, suggested_name: &str) -> Result<Option<PathBuf>, AppError> {
        let dialog = self
            .app
            .dialog()
            .file()
            .add_filter("Paczka Alfy", &["alfa"])
            .set_file_name(suggested_name);
        picked(dialog.blocking_save_file())
    }

    fn pick_open_path(&self) -> Result<Option<PathBuf>, AppError> {
        let dialog = self
            .app
            .dialog()
            .file()
            .add_filter("Paczka Alfy", &["alfa"]);
        picked(dialog.blocking_pick_file())
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
            Some(ProtocolAction::OpenSession(id)) => core.open_session_in_ui(id).await,
            Some(ProtocolAction::NewChat(text)) => new_chat(&core, text).await,
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "akcja protokołu alfa:// nie powiodła się");
            toast(&core, &e.message, "The alfa:// action failed.");
        }
    });
}

/// Nowa rozmowa (zasobnik, protokół): sesja + szkic (tekst z zewnątrz nie jest wysyłany),
/// UI przechodzi do niej (`OpenSession`), okno główne na wierzch.
pub async fn new_chat(core: &AppCore, text: Option<String>) -> Result<(), AppError> {
    let session = core.sessions_create(SessionTemplate::Empty).await?;
    if let Some(text) = text {
        core.sessions_save_draft(session.id.clone(), text).await?;
    }
    core.open_session_in_ui(session.id).await
}
