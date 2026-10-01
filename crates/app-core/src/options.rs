//! Ścieżki danych i opcje budowy `AppCore` (porty modułów do podmiany w testach i kolejnych falach).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use accounts_hub_contract::SecretStore;
use device_profile_contract::DeviceProfile as DeviceProfileService;

use crate::error::AppError;
use crate::events::DEFAULT_FRAME;
use crate::ports::{BrainPort, BrokerPort, ShellPort, TransferPort, VoicePort};

/// Katalogi aplikacji (ADR 0007, PLAN §15):
/// `%APPDATA%\Alfa\config`, `%LOCALAPPDATA%\Alfa\{sessions,models,logs,state,webview-data}`,
/// katalogi robocze sesji `%USERPROFILE%\Alfa\Sesje`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// Konfiguracja (`*.toml`, `machine/<id>.toml`, `accounts.json`).
    pub config: PathBuf,
    /// Dane lokalne (`%LOCALAPPDATA%\Alfa`).
    pub local: PathBuf,
    /// Korzeń plików użytkownika (`%USERPROFILE%\Alfa`; sesje w `Sesje\<nazwa>`).
    pub user_root: PathBuf,
}

impl AppPaths {
    /// Ścieżki ze zmiennych środowiskowych Windows (`APPDATA`, `LOCALAPPDATA`, `USERPROFILE`);
    /// poza Windows — `~/.config/alfa`, `~/.local/share/alfa`, `~/Alfa` (tryb deweloperski).
    pub fn from_env() -> Result<Self, AppError> {
        let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
        if cfg!(windows) {
            let missing = |n: &str| AppError::internal(format!("brak zmiennej środowiskowej {n}"));
            let appdata = var("APPDATA").ok_or_else(|| missing("APPDATA"))?;
            let local = var("LOCALAPPDATA").ok_or_else(|| missing("LOCALAPPDATA"))?;
            let profile = var("USERPROFILE").ok_or_else(|| missing("USERPROFILE"))?;
            return Ok(Self {
                config: appdata.join("Alfa").join("config"),
                local: local.join("Alfa"),
                user_root: profile.join("Alfa"),
            });
        }
        let home = var("HOME").ok_or_else(|| AppError::internal("brak zmiennej HOME"))?;
        Ok(Self {
            config: home.join(".config").join("alfa"),
            local: home.join(".local").join("share").join("alfa"),
            user_root: home.join("Alfa"),
        })
    }

    /// Wszystko pod jednym katalogiem (testy, tryb przenośny).
    pub fn under(root: &Path) -> Self {
        Self {
            config: root.join("config"),
            local: root.join("local"),
            user_root: root.join("user"),
        }
    }

    /// Bazy sesji.
    pub fn sessions(&self) -> PathBuf {
        self.local.join("sessions")
    }

    /// Modele lokalne.
    pub fn models(&self) -> PathBuf {
        self.local.join("models")
    }

    /// Logi NDJSON.
    pub fn logs(&self) -> PathBuf {
        self.local.join("logs")
    }

    /// Stan (zapasowy `MachineId`, dziennik kosztów).
    pub fn state(&self) -> PathBuf {
        self.local.join("state")
    }

    /// Stały folder danych WebView2 (poza katalogiem wersji — ADR 0007).
    pub fn webview_data(&self) -> PathBuf {
        self.local.join("webview-data")
    }

    /// Katalogi robocze sesji.
    pub fn workdirs(&self) -> PathBuf {
        self.user_root.join("Sesje")
    }

    /// Tworzy wszystkie katalogi.
    pub fn ensure(&self) -> Result<(), AppError> {
        for dir in [
            self.config.clone(),
            self.sessions(),
            self.models(),
            self.logs(),
            self.state(),
            self.webview_data(),
            self.workdirs(),
        ] {
            std::fs::create_dir_all(&dir)
                .map_err(|e| AppError::storage(format!("{}: {e}", dir.display())))?;
        }
        Ok(())
    }
}

/// Opcje budowy. `None` w porcie = domyślna implementacja (produkcyjna albo „niepodłączony moduł").
pub struct AppOptions {
    /// Wersja aplikacji (do `app_bootstrap`).
    pub app_version: String,
    /// Długość klatki paczek zdarzeń (§14.7).
    pub frame: Duration,
    /// Okno cofnięcia usunięcia sesji (domyślnie z ustawień, 10 s).
    pub undo_window: Option<Duration>,
    /// Pobieranie kursu NBP (wyłączone → kurs zapasowy).
    pub fetch_fx: bool,
    /// Magazyn sekretów (`None` = Credential Manager; poza Windows — pamięć procesu).
    pub secrets: Option<Arc<dyn SecretStore>>,
    /// Profil urządzenia (`None` = detekcja sprzętu).
    pub device: Option<Arc<dyn DeviceProfileService>>,
    /// Wybór modelu (`None` = pierwszy skonfigurowany dostawca; docelowo `router`).
    pub brain: Option<Arc<dyn BrainPort>>,
    /// Import/eksport (`None` = moduł `transfer` niepodłączony).
    pub transfer: Option<Arc<dyn TransferPort>>,
    /// Głos (`None` = moduły `voice-*` niepodłączone).
    pub voice: Option<Arc<dyn VoicePort>>,
    /// Broker (`None` = `safety-broker` niepodłączony).
    pub broker: Option<Arc<dyn BrokerPort>>,
    /// Powłoka (`None` = bez okien).
    pub shell: Option<Arc<dyn ShellPort>>,
    /// Zapis logów NDJSON z magistrali (`core-log`).
    pub file_logs: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            frame: DEFAULT_FRAME,
            undo_window: None,
            fetch_fx: true,
            secrets: None,
            device: None,
            brain: None,
            transfer: None,
            voice: None,
            broker: None,
            shell: None,
            file_logs: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_under_root_follow_layout() {
        let p = AppPaths::under(Path::new("/x"));
        assert_eq!(p.sessions(), Path::new("/x/local/sessions"));
        assert_eq!(p.webview_data(), Path::new("/x/local/webview-data"));
        assert_eq!(p.workdirs(), Path::new("/x/user/Sesje"));
    }
}
