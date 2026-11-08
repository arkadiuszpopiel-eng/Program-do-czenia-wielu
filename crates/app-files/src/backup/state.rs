//! Stan kopii zapasowych tej maszyny (`%LOCALAPPDATA%\Alfa\state\backup.json`): ustawienia,
//! czas i wynik ostatniej kopii. Plik lokalny — nie trafia do paczek `.alfa` (katalog kopii jest
//! ścieżką tej maszyny). Zapis atomowy (plik tymczasowy + zamiana).

use std::path::{Path, PathBuf};

use app_api::dto::BackupConfig;
use app_api::error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Najkrótszy i najdłuższy odstęp między kopiami (godziny).
pub const INTERVAL_HOURS: (u32, u32) = (1, 24 * 30);
/// Najmniej i najwięcej zachowanych kopii.
pub const KEEP: (u32, u32) = (1, 100);

/// Ustawienia domyślne: wyłączone, codziennie, 7 kopii, bez artefaktów i logów, nie na baterii.
pub fn default_config() -> BackupConfig {
    BackupConfig {
        enabled: false,
        dir: None,
        interval_hours: 24,
        keep: 7,
        include_artifacts: false,
        include_logs: false,
        skip_on_battery: true,
    }
}

/// Zapisany stan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupState {
    /// Ustawienia.
    pub config: BackupConfig,
    /// Ostatnia udana kopia.
    pub last_run: Option<DateTime<Utc>>,
    /// Błąd ostatniej próby (tekst dla UI, bez ścieżek prywatnych innych niż katalog kopii).
    pub last_error: Option<String>,
}

impl Default for BackupState {
    fn default() -> Self {
        Self {
            config: default_config(),
            last_run: None,
            last_error: None,
        }
    }
}

impl BackupState {
    /// Odczyt; brak albo uszkodzony plik — stan domyślny (kopie wyłączone).
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<BackupState>(&bytes).ok())
            .map(|mut s| {
                s.config = sanitize(s.config);
                s
            })
            .unwrap_or_default()
    }

    /// Zapis atomowy.
    pub fn save(&self, path: &Path) -> Result<(), AppError> {
        let storage = |e: std::io::Error| AppError::storage(format!("{}: {e}", path.display()));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(storage)?;
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| AppError::internal(format!("stan kopii: {e}")))?;
        let tmp = temp_path(path);
        std::fs::write(&tmp, bytes).map_err(storage)?;
        std::fs::rename(&tmp, path).map_err(storage)
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Wartości w dozwolonych zakresach; włączenie wymaga katalogu.
pub fn sanitize(mut c: BackupConfig) -> BackupConfig {
    c.interval_hours = c.interval_hours.clamp(INTERVAL_HOURS.0, INTERVAL_HOURS.1);
    c.keep = c.keep.clamp(KEEP.0, KEEP.1);
    if c.dir.as_deref().is_none_or(str::is_empty) {
        c.dir = None;
        c.enabled = false;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trips_and_broken_file_means_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("backup.json");
        assert_eq!(BackupState::load(&path), BackupState::default());
        let state = BackupState {
            config: BackupConfig {
                enabled: true,
                dir: Some("/kopie".into()),
                ..default_config()
            },
            last_run: Some(Utc::now()),
            last_error: None,
        };
        state.save(&path).unwrap();
        assert_eq!(BackupState::load(&path), state);
        std::fs::write(&path, b"{zly").unwrap();
        assert_eq!(BackupState::load(&path), BackupState::default());
    }

    #[test]
    fn config_is_clamped_and_needs_dir() {
        let c = sanitize(BackupConfig {
            enabled: true,
            dir: Some(String::new()),
            interval_hours: 0,
            keep: 1000,
            ..default_config()
        });
        assert!(!c.enabled);
        assert_eq!(c.dir, None);
        assert_eq!((c.interval_hours, c.keep), (1, 100));
    }
}
