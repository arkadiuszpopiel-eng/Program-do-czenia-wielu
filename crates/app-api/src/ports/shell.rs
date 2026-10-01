//! Port powłoki (Tauri): okna, natywne dialogi i akcje systemowe wykonywane jako użytkownik.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::ArtifactIntent;
use crate::error::AppError;

const SHELL: &str = "shell-integration";

/// Powłoka (Tauri): okna, natywne dialogi i akcje systemowe wykonywane jako użytkownik.
/// Metody dialogów blokują wątek — rdzeń woła je przez `spawn_blocking`.
pub trait ShellPort: Send + Sync {
    /// Pokazuje okno główne (opcjonalnie z sesją).
    fn show_main(&self, session: Option<&str>) -> Result<(), AppError>;
    /// Chowa okno Szybkiego pytania.
    fn hide_quick(&self) -> Result<(), AppError>;
    /// Otwiera stronę ustawień Windows (URI już sprawdzony z listą dozwolonych).
    fn open_system_settings(&self, uri: &str) -> Result<(), AppError>;
    /// Wykonuje zwalidowaną intencję pliku (Otwórz, Pokaż w Eksploratorze, Kopiuj, Zapisz jako).
    fn artifact_action(&self, intent: &ArtifactIntent) -> Result<(), AppError>;
    /// Natywny dialog „Zapisz jako" dla tekstu; `false` = anulowano.
    fn save_text_as(&self, suggested_name: &str, text: &str) -> Result<bool, AppError>;
    /// Natywny dialog „Zapisz jako" dla pliku (np. paczka `.alfa`); `None` = anulowano.
    fn pick_save_path(&self, _suggested_name: &str) -> Result<Option<PathBuf>, AppError> {
        Err(AppError::unavailable("Okno wyboru pliku", SHELL))
    }
    /// Natywny dialog „Otwórz" (paczka `.alfa`); `None` = anulowano.
    fn pick_open_path(&self) -> Result<Option<PathBuf>, AppError> {
        Err(AppError::unavailable("Okno wyboru pliku", SHELL))
    }
    /// Wolne miejsce na dysku z `path` (`None` = nieznane).
    fn disk_free(&self, _path: &Path) -> Option<u64> {
        None
    }
}

/// Powłoka bez okien (testy, tryb bezgłowy): zapisuje wywołania; odpowiedzi dialogów wybiera
/// test ([`HeadlessShell::answer_dialog`]), bez nich dialog jest niedostępny.
#[derive(Default)]
pub struct HeadlessShell {
    calls: Mutex<Vec<String>>,
    dialogs: Mutex<VecDeque<Option<PathBuf>>>,
}

impl HeadlessShell {
    /// Zarejestrowane wywołania (do asercji w testach).
    pub fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Kolejkuje odpowiedź następnego dialogu pliku (`None` = użytkownik anulował).
    pub fn answer_dialog(&self, answer: Option<PathBuf>) {
        self.dialogs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(answer);
    }

    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call);
    }

    fn dialog(&self, call: String) -> Result<Option<PathBuf>, AppError> {
        self.record(call);
        self.dialogs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .ok_or_else(|| AppError::unavailable("Okno wyboru pliku", SHELL))
    }
}

impl ShellPort for HeadlessShell {
    fn show_main(&self, session: Option<&str>) -> Result<(), AppError> {
        self.record(format!("show_main:{}", session.unwrap_or("-")));
        Ok(())
    }
    fn hide_quick(&self) -> Result<(), AppError> {
        self.record("hide_quick".into());
        Ok(())
    }
    fn open_system_settings(&self, uri: &str) -> Result<(), AppError> {
        self.record(format!("open_system_settings:{uri}"));
        Ok(())
    }
    fn artifact_action(&self, intent: &ArtifactIntent) -> Result<(), AppError> {
        self.record(format!("artifact:{}:{:?}", intent.artifact, intent.action));
        Ok(())
    }
    fn save_text_as(&self, _suggested_name: &str, _text: &str) -> Result<bool, AppError> {
        Err(AppError::unavailable("Zapis przez okno dialogowe", SHELL))
    }
    fn pick_save_path(&self, suggested_name: &str) -> Result<Option<PathBuf>, AppError> {
        self.dialog(format!("pick_save:{suggested_name}"))
    }
    fn pick_open_path(&self) -> Result<Option<PathBuf>, AppError> {
        self.dialog("pick_open".into())
    }
}
