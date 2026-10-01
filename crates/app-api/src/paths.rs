//! Katalogi aplikacji (ADR 0007, PLAN §15) — wspólne dla rdzenia, adapterów modułów i powłoki.

use std::path::{Path, PathBuf};

use crate::error::AppError;

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

    /// Sidecary pobrane w Ustawieniach (`llama-server`, Piper, Pocket TTS).
    pub fn sidecars(&self) -> PathBuf {
        self.local.join("sidecars")
    }

    /// Snapshoty przed importem `.alfa` (rollback jednym kliknięciem).
    pub fn snapshots(&self) -> PathBuf {
        self.local.join("snapshots")
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
            self.sidecars(),
            self.snapshots(),
        ] {
            std::fs::create_dir_all(&dir)
                .map_err(|e| AppError::storage(format!("{}: {e}", dir.display())))?;
        }
        Ok(())
    }
}

/// Nazwa pliku wykonywalnego dla systemu.
fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

impl AppPaths {
    /// Plik sidecara `<silnik>/<plik>`: najpierw dane lokalne (silnik pobrany w Ustawieniach),
    /// potem katalog programu (`<katalog exe>\sidecars\…`, paczka wydania); gdy żadnego nie ma —
    /// ścieżka w danych lokalnych (komunikat błędu wskaże, gdzie go umieścić).
    pub fn sidecar(&self, engine: &str, file: &str) -> PathBuf {
        let local = self.sidecars().join(engine).join(exe(file));
        if local.is_file() {
            return local;
        }
        let bundled = std::env::current_exe().ok().and_then(|p| {
            p.parent()
                .map(|d| d.join("sidecars").join(engine).join(exe(file)))
        });
        match bundled {
            Some(path) if path.is_file() => path,
            _ => local,
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
