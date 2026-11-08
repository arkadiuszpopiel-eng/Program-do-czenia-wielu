//! Usługi procesów Jądra (PLAN §8.1, ADR 3): host usługi Windows (Broker w sesji 0 na osobnym
//! koncie), uruchamianie procesu w sesji interaktywnego użytkownika z wyższym poziomem
//! integralności (Broker-UI) i katalogi prywatne z chronionym DACL (kotwica i pliki Audytu).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;
use crate::peer::Sid;

/// Sygnał zatrzymania usługi (wspólny między hostem a ciałem usługi).
#[derive(Debug, Clone, Default)]
pub struct StopSignal(Arc<(Mutex<bool>, Condvar)>);

impl StopSignal {
    /// Nowy, niezatrzymany.
    pub fn new() -> Self {
        Self::default()
    }

    /// Zgłasza zatrzymanie i budzi czekających.
    pub fn stop(&self) {
        let (lock, cv) = &*self.0;
        *lock.lock().unwrap_or_else(|p| p.into_inner()) = true;
        cv.notify_all();
    }

    /// Czy zgłoszono zatrzymanie.
    pub fn is_stopped(&self) -> bool {
        *self.0.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Czeka na zatrzymanie co najwyżej `timeout`; `true` = zatrzymano.
    pub fn wait(&self, timeout: Duration) -> bool {
        let (lock, cv) = &*self.0;
        let guard = lock.lock().unwrap_or_else(|p| p.into_inner());
        let (guard, _) = cv
            .wait_timeout_while(guard, timeout, |stopped| !*stopped)
            .unwrap_or_else(|p| p.into_inner());
        *guard
    }
}

/// Ciało usługi: działa do zatrzymania; błąd = usługa kończy się ze stanem błędu.
pub type ServiceBody = Box<dyn FnOnce(StopSignal) -> Result<(), String> + Send>;

/// Host usługi systemowej.
pub trait ServiceHostPort: Send + Sync {
    /// Rejestruje proces w menedżerze usług pod nazwą `name` i uruchamia `body` (blokuje do
    /// zatrzymania). Poza menedżerem usług → błąd (wtedy tryb konsolowy).
    fn run_service(&self, name: &str, body: ServiceBody) -> Result<(), PlatformError>;
}

/// Poziom integralności uruchamianego procesu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchIntegrity {
    /// Jak wywołujący, w jego sesji (tryb deweloperski — bez ochrony UIPI).
    AsCaller,
    /// Sesja konsoli interaktywnego użytkownika, token użytkownika z podniesioną etykietą
    /// `High` (bez grup administratora) — UIPI blokuje SendInput z procesów średniej i niskiej
    /// integralności. Wymaga usługi z `SeTcbPrivilege` (bramka ludzka #10).
    UserSessionHigh,
}

/// Specyfikacja uruchomienia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLaunch {
    /// Ścieżka bezwzględna obrazu.
    pub image: PathBuf,
    /// Argumenty.
    pub args: Vec<String>,
    /// Poziom integralności.
    pub integrity: LaunchIntegrity,
    /// Dane przekazywane przez stdin (np. poświadczenie IPC) — nigdy przez wiersz poleceń.
    pub stdin: Vec<u8>,
}

/// Uruchamianie procesów pomocniczych Jądra (Broker-UI).
pub trait SessionLauncherPort: Send + Sync {
    /// Uruchamia proces; zwraca PID.
    fn launch(&self, spec: &SessionLaunch) -> Result<u32, PlatformError>;

    /// Czy proces uruchomiony przez ten port nadal działa (po uchwycie — bez wyścigu PID).
    fn is_running(&self, pid: u32) -> bool;
}

/// Katalogi z chronionym DACL (SDDL: `pipe::private_dir_sddl`).
pub trait PrivateDirPort: Send + Sync {
    /// Tworzy katalog albo wymusza na istniejącym chroniony DACL: pełny dostęp tylko `owner`
    /// i `SYSTEM`, dziedziczony przez pliki.
    fn ensure_private_dir(&self, path: &Path, owner: &Sid) -> Result<(), PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_signal_wakes_waiters() {
        let s = StopSignal::new();
        assert!(!s.is_stopped());
        assert!(!s.wait(Duration::from_millis(1)));
        let t = s.clone();
        let h = std::thread::spawn(move || t.wait(Duration::from_secs(5)));
        s.stop();
        assert!(h.join().unwrap());
        assert!(s.is_stopped());
        assert!(s.wait(Duration::ZERO));
    }
}
