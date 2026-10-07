//! Narzędzia aplikacji F6 i wtyczki dla agentek: Word/Excel (`tools-office` nad `OfficePort`),
//! izolowana przeglądarka (`tools-browser` nad `BrowserPort`, profil i kwarantanna w katalogu
//! danych Alfy) oraz wtyczki Wasm (`app-plugins`, narzędzia odświeżane przy każdym odczycie
//! rejestru). Porty systemowe: `WinOffice` (kopie robocze w `%LOCALAPPDATA%\Alfa\office-work`)
//! i `CdpBrowser::system` — poza Windows oba zwracają „nieobsługiwane”/„niedostępna”, więc
//! narzędzia odpowiadają czytelnym błędem bez skutków.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use platform_apps_contract::{BrowserKind, BrowserPort, BrowserSpec, OfficePort};
use platform_windows_office_impl::{BrowserConfig, CdpBrowser, OfficeConfig, WinOffice};

/// Porty aplikacji i katalog wtyczek.
#[derive(Clone)]
pub struct AppsDeps {
    /// Word/Excel (COM na wątku STA; atrapa w testach).
    pub office: Arc<dyn OfficePort>,
    /// Przeglądarka Alfy (CDP przez potok; atrapa w testach).
    pub browser: Arc<dyn BrowserPort>,
    /// Profil i kwarantanna przeglądarki (wewnątrz katalogu danych Alfy).
    pub browser_spec: BrowserSpec,
    /// Katalog magazynu wtyczek (`None` — bez wtyczek).
    pub plugins_dir: Option<PathBuf>,
    /// F6: porty `tools-system` i `tools-net` (`None` — bez narzędzi systemowych i sieciowych).
    pub sysnet: Option<crate::sysnet::SysNetDeps>,
}

impl std::fmt::Debug for AppsDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppsDeps")
            .field("browser_spec", &self.browser_spec)
            .field("plugins_dir", &self.plugins_dir)
            .field("sysnet", &self.sysnet)
            .finish_non_exhaustive()
    }
}

impl AppsDeps {
    /// Porty systemowe nad katalogiem danych Alfy (`%LOCALAPPDATA%\Alfa`).
    pub fn system(local: &Path) -> Self {
        Self {
            office: Arc::new(WinOffice::new(OfficeConfig::new(local.join("office-work")))),
            browser: Arc::new(CdpBrowser::system(BrowserConfig::default())),
            browser_spec: browser_spec(local),
            plugins_dir: Some(local.join("plugins")),
            sysnet: Some(crate::sysnet::SysNetDeps::system(local)),
        }
    }
}

/// Przeglądarka Alfy: Edge (domyślna w Windows 11) bez okna, profil `browser\profile`
/// i kwarantanna pobrań `browser\quarantine` w katalogu danych Alfy — nigdy profil użytkownika.
pub fn browser_spec(local: &Path) -> BrowserSpec {
    let root = local.join("browser");
    BrowserSpec {
        kind: BrowserKind::Edge,
        executable: None,
        alfa_root: local.to_path_buf(),
        profile_dir: root.join("profile"),
        quarantine_dir: root.join("quarantine"),
        headless: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_profile_and_quarantine_live_in_alfa_dir() {
        let local = Path::new(r"C:\Users\ala\AppData\Local\Alfa");
        let spec = browser_spec(local);
        assert!(spec.profile_dir.starts_with(local));
        assert!(spec.quarantine_dir.starts_with(local));
        assert_ne!(spec.profile_dir, spec.quarantine_dir);
        assert!(spec.headless);
        let deps = AppsDeps::system(local);
        assert_eq!(deps.plugins_dir, Some(local.join("plugins")));
        assert!(format!("{deps:?}").contains("quarantine"));
    }

    #[test]
    fn system_spec_is_valid_for_an_absolute_root() {
        let root = std::env::temp_dir().join("alfa-spec");
        assert!(browser_spec(&root).validate().is_ok());
    }
}
