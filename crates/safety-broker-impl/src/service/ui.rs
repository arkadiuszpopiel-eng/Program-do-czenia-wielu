//! Nadzór Broker-UI: usługa uruchamia okno zatwierdzeń w sesji interaktywnego użytkownika
//! z wyższym poziomem integralności (`LaunchIntegrity::UserSessionHigh`) i przekazuje mu bilet
//! startowy przez stdin (poświadczenie roli `BrokerUi` — nowe przy każdym uruchomieniu; nigdy
//! w wierszu poleceń). Po zakończeniu procesu — ponowne uruchomienie z rosnącą przerwą.

use std::path::PathBuf;

use platform_contract::{LaunchIntegrity, SessionLaunch, SessionLauncherPort};
use safety_broker_contract::ipc::ClientRole;
use safety_broker_contract::ipc_blocking::UiLaunchTicket;
use serde::{Deserialize, Serialize};

use crate::engine::BrokerEngine;

/// Konfiguracja uruchamiania Broker-UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiLaunchConfig {
    /// Ścieżka `alfa-broker-ui.exe` (Program Files — zapis tylko dla administratorów).
    pub image: PathBuf,
    /// Argumenty.
    #[serde(default)]
    pub args: Vec<String>,
    /// Poziom integralności (produkcyjnie `user_session_high`).
    pub integrity: LaunchIntegrity,
    /// Ważność poświadczenia (ms).
    pub credential_ttl_ms: u64,
    /// Początkowa przerwa przed ponownym uruchomieniem (ms).
    pub restart_backoff_ms: u64,
}

/// Maksymalna przerwa między uruchomieniami (ms).
const MAX_BACKOFF_MS: u64 = 30_000;
/// Proces działający dłużej niż tyle uznajemy za zdrowy (reset przerwy).
const HEALTHY_AFTER_MS: u64 = 10_000;
/// Co ile sprawdzamy, czy proces żyje (ms).
const CHECK_MS: u64 = 500;

/// Stan nadzoru (czysta logika kroku — testowalna na atrapach).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSupervisor {
    pid: Option<u32>,
    started_at_ms: u64,
    backoff_ms: u64,
    launches: u32,
}

impl UiSupervisor {
    /// Nowy nadzór.
    pub fn new(config: &UiLaunchConfig) -> Self {
        Self {
            pid: None,
            started_at_ms: 0,
            backoff_ms: config.restart_backoff_ms.clamp(100, MAX_BACKOFF_MS),
            launches: 0,
        }
    }

    /// PID działającego Broker-UI.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Liczba uruchomień.
    pub fn launches(&self) -> u32 {
        self.launches
    }

    /// Jeden krok; zwraca, ile ms czekać do następnego (i ewentualny błąd do dziennika).
    pub fn step(
        &mut self,
        engine: &BrokerEngine,
        launcher: &dyn SessionLauncherPort,
        config: &UiLaunchConfig,
        ticket_base: &UiLaunchTicket,
        now_ms: u64,
    ) -> UiStep {
        let ok = |wait_ms| UiStep {
            wait_ms,
            error: None,
        };
        if let Some(pid) = self.pid {
            if launcher.is_running(pid) {
                return ok(CHECK_MS);
            }
            self.pid = None;
            if now_ms.saturating_sub(self.started_at_ms) >= HEALTHY_AFTER_MS {
                self.backoff_ms = config.restart_backoff_ms.clamp(100, MAX_BACKOFF_MS);
            }
            return ok(self.next_backoff());
        }
        // Każde uruchomienie dostaje własne poświadczenie (inny identyfikator klienta).
        let client_id = format!("broker-ui-{}", self.launches + 1);
        let credential = engine.issue_client_credential(
            &client_id,
            ClientRole::BrokerUi,
            config.credential_ttl_ms,
        );
        let ticket = UiLaunchTicket {
            credential,
            ..ticket_base.clone()
        };
        let mut stdin = match serde_json::to_vec(&ticket) {
            Ok(bytes) => bytes,
            Err(e) => {
                return UiStep {
                    wait_ms: self.next_backoff(),
                    error: Some(format!("bilet Broker-UI: {e}")),
                };
            }
        };
        stdin.push(b'\n');
        let spec = SessionLaunch {
            image: config.image.clone(),
            args: config.args.clone(),
            integrity: config.integrity,
            stdin,
        };
        match launcher.launch(&spec) {
            Ok(pid) => {
                self.pid = Some(pid);
                self.started_at_ms = now_ms;
                self.launches += 1;
                ok(CHECK_MS)
            }
            Err(e) => {
                let wait_ms = self.next_backoff();
                UiStep {
                    wait_ms,
                    error: Some(format!(
                        "uruchomienie Broker-UI nieudane ({e}); ponowna próba za {wait_ms} ms"
                    )),
                }
            }
        }
    }

    fn next_backoff(&mut self) -> u64 {
        let wait = self.backoff_ms;
        self.backoff_ms = (self.backoff_ms * 2).min(MAX_BACKOFF_MS);
        wait
    }
}

/// Wynik kroku nadzoru.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiStep {
    /// Ile czekać do następnego kroku (ms).
    pub wait_ms: u64,
    /// Błąd do dziennika (usługa działa dalej).
    pub error: Option<String>,
}
