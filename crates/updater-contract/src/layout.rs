//! Układ katalogów instalacji (ADR 0007): `%LOCALAPPDATA%\Alfa\` z wersjami side-by-side,
//! stałym launcherem, `current.json` i stałym folderem danych WebView2 poza katalogami wersji.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Stały launcher (jedyna ścieżka znana skrótom, AUMID, autostartowi, protokołowi, „Wyślij do”).
pub const LAUNCHER_EXE: &str = "alfa.exe";
/// Plik wykonywalny aplikacji w katalogu wersji.
pub const APP_EXE: &str = "alfa-desktop.exe";
/// Katalog wersji.
pub const VERSIONS_DIR: &str = "versions";
/// Wskaźnik wersji aktywnej i poprzedniej.
pub const CURRENT_FILE: &str = "current.json";
/// Stały folder danych WebView2 (przeżywa aktualizacje).
pub const WEBVIEW_DIR: &str = "webview-data";
/// Opcjonalny opis wersji w jej katalogu (`{ "version": "…" }`).
pub const VERSION_FILE: &str = "version.json";
/// Nazwa katalogu Alfy w `%LOCALAPPDATA%`.
pub const ROOT_DIR: &str = "Alfa";

/// Ścieżki instalacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Layout {
    /// `%LOCALAPPDATA%\Alfa`.
    pub root: PathBuf,
    /// `<root>\alfa.exe`.
    pub launcher: PathBuf,
    /// `<root>\versions`.
    pub versions: PathBuf,
    /// `<root>\webview-data`.
    pub webview_data: PathBuf,
    /// `<root>\current.json`.
    pub current: PathBuf,
}

impl Layout {
    /// Układ w katalogu `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            launcher: root.join(LAUNCHER_EXE),
            versions: root.join(VERSIONS_DIR),
            webview_data: root.join(WEBVIEW_DIR),
            current: root.join(CURRENT_FILE),
            root,
        }
    }

    /// Układ w `%LOCALAPPDATA%\Alfa` (ścieżkę `%LOCALAPPDATA%` podaje wywołujący).
    pub fn for_local_app_data(local_app_data: &Path) -> Self {
        Self::new(local_app_data.join(ROOT_DIR))
    }

    /// Katalog wersji.
    pub fn version_dir(&self, version: &semver::Version) -> PathBuf {
        self.versions.join(version.to_string())
    }

    /// Plik wykonywalny aplikacji danej wersji.
    pub fn app_exe(&self, version: &semver::Version) -> PathBuf {
        self.version_dir(version).join(APP_EXE)
    }

    /// Czy ścieżka leży w katalogu wersji (rejestracje systemowe **nigdy** nie mogą tam wskazywać).
    pub fn is_inside_versions(&self, path: &Path) -> bool {
        path.starts_with(&self.versions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_paths_are_outside_version_dirs() {
        let l = Layout::for_local_app_data(Path::new("/lad"));
        assert_eq!(l.root, Path::new("/lad/Alfa"));
        assert_eq!(l.launcher, Path::new("/lad/Alfa/alfa.exe"));
        let v = semver::Version::new(0, 3, 1);
        assert_eq!(
            l.app_exe(&v),
            Path::new("/lad/Alfa/versions/0.3.1/alfa-desktop.exe")
        );
        assert!(!l.is_inside_versions(&l.launcher));
        assert!(!l.is_inside_versions(&l.webview_data));
        assert!(!l.is_inside_versions(&l.current));
        assert!(l.is_inside_versions(&l.app_exe(&v)));
    }
}
