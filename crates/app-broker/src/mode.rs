//! Wybór trybu Brokera przy starcie aplikacji (ADR 0003):
//!
//! 1. **Usługa** — istnieje konfiguracja usługi `AlfaBroker` (`%ProgramData%\Alfa\broker\broker.json`,
//!    zapisywana przy instalacji usługi — bramka ludzka #10): łączymy się z jej potokiem. Gdy
//!    usługa nie działa albo konfiguracja jest uszkodzona — **bez** cichego przejścia na tryb
//!    przenośny (obniżenie izolacji byłoby atakiem): bezpieczny stan z wyjaśnieniem.
//! 2. **Przenośny** — obok aplikacji leży `alfa-broker(.exe)`: aplikacja uruchamia go jako proces
//!    potomny `--console` (konto użytkownika, słabsza izolacja — oznaczone w UI); okno Broker-UI
//!    uruchamia sam Broker.
//! 3. **W procesie** — brak binarek Jądra (build deweloperski, Linux/CI): dotychczasowy Broker
//!    w procesie, bez okna zatwierdzeń.

use std::path::{Path, PathBuf};

use platform_contract::Sid;
use safety_broker_impl::service::ServiceConfig;

/// Potok trybu przenośnego (`alfa-broker --console`, `app_safety::broker::DEV_PIPE`).
pub const PORTABLE_PIPE: &str = "alfa-broker-dev";

/// Nazwa pliku wykonywalnego Jądra w katalogu aplikacji (`alfa-broker` → `alfa-broker.exe`).
pub fn image_name(stem: &str) -> String {
    format!("{stem}{}", std::env::consts::EXE_SUFFIX)
}

/// Ścieżka konfiguracji usługi w katalogu danych programów (`%ProgramData%`).
pub fn service_config_path(program_data: &Path) -> PathBuf {
    program_data.join("Alfa").join("broker").join("broker.json")
}

/// Cel połączenia z usługą.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTarget {
    /// Potok usługi.
    pub pipe: String,
    /// Konto usługi (serwer potoku musi na nim działać).
    pub broker_user: Sid,
    /// Czy usługa uruchamia Broker-UI.
    pub window: bool,
}

/// Wykryty tryb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detected {
    /// Usługa zainstalowana (albo jej konfiguracja jest uszkodzona — wtedy błąd).
    Service(Result<ServiceTarget, String>),
    /// Tryb przenośny: binarki Jądra obok aplikacji.
    Portable {
        /// `alfa-broker(.exe)`.
        broker: PathBuf,
        /// Czy obok jest `alfa-broker-ui(.exe)` (inaczej prośby czekają do wygaśnięcia).
        window: bool,
    },
    /// Broker w procesie (powód po polsku).
    InProcess(String),
}

/// `alfa-watchdog(.exe)` obok aplikacji, jeśli jest.
pub fn watchdog_image(exe_dir: &Path) -> Option<PathBuf> {
    let path = exe_dir.join(image_name("alfa-watchdog"));
    path.is_file().then_some(path)
}

/// Wykrywa tryb: `exe_dir` — katalog aplikacji (wersji), `service_config` — `broker.json` usługi.
pub fn detect(exe_dir: &Path, service_config: Option<&Path>) -> Detected {
    if let Some(path) = service_config.filter(|p| p.exists()) {
        return Detected::Service(read_service(path));
    }
    let broker = exe_dir.join(image_name("alfa-broker"));
    if broker.is_file() {
        let window = exe_dir.join(image_name("alfa-broker-ui")).is_file();
        return Detected::Portable { broker, window };
    }
    Detected::InProcess(format!(
        "brak {} obok aplikacji ({})",
        image_name("alfa-broker"),
        exe_dir.display()
    ))
}

fn read_service(path: &Path) -> Result<ServiceTarget, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("konfiguracja usługi Brokera {}: {e}", path.display()))?;
    let config: ServiceConfig = serde_json::from_str(&text)
        .map_err(|e| format!("konfiguracja usługi Brokera {}: {e}", path.display()))?;
    config.validate()?;
    Ok(ServiceTarget {
        pipe: config.pipe_name,
        broker_user: config.broker_user,
        window: config.broker_ui.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_in_process_portable_and_service() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(detect(dir.path(), None), Detected::InProcess(_)));
        std::fs::write(dir.path().join(image_name("alfa-broker")), b"").unwrap();
        match detect(dir.path(), None) {
            Detected::Portable { broker, window } => {
                assert!(broker.ends_with(image_name("alfa-broker")));
                assert!(!window);
            }
            other => panic!("{other:?}"),
        }
        std::fs::write(dir.path().join(image_name("alfa-broker-ui")), b"").unwrap();
        assert!(matches!(
            detect(dir.path(), None),
            Detected::Portable { window: true, .. }
        ));
        assert!(watchdog_image(dir.path()).is_none());
        std::fs::write(dir.path().join(image_name("alfa-watchdog")), b"").unwrap();
        assert!(watchdog_image(dir.path()).is_some());
        // Konfiguracja usługi ma pierwszeństwo — uszkodzona nie przechodzi na tryb przenośny.
        let cfg = service_config_path(dir.path());
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, b"{ zepsute").unwrap();
        assert!(matches!(
            detect(dir.path(), Some(&cfg)),
            Detected::Service(Err(_))
        ));
        assert!(matches!(
            detect(dir.path(), Some(&dir.path().join("brak.json"))),
            Detected::Portable { .. }
        ));
    }
}
