//! Port `watchdog-contract::UpdaterSignal` na module plików: watchdog po pętli awarii zleca
//! rollback wersji do „ostatniej dobrej” — przełączamy `current.json` na poprzednią (porzucona
//! wersja trafia do wycofanych). Nowa wersja działa od następnego startu przez launcher.

use std::sync::Arc;

use semver::Version;
use updater_contract::Updater;
use watchdog_contract::UpdaterSignal;

use crate::FsUpdater;

/// Sygnał rollbacku od watchdoga.
pub struct WatchdogSignal {
    updater: Arc<FsUpdater>,
    running: Version,
}

impl WatchdogSignal {
    /// Sygnał dla modułu plików; `running` — wersja uruchomiona (gdy brak `current.json`).
    pub fn new(updater: Arc<FsUpdater>, running: Version) -> Self {
        Self { updater, running }
    }
}

impl UpdaterSignal for WatchdogSignal {
    fn current_version(&self) -> String {
        match self.updater.state() {
            Ok(Some(s)) => s.active.to_string(),
            _ => self.running.to_string(),
        }
    }

    fn request_rollback(&self, to: &str) -> Result<(), String> {
        let to = Version::parse(to).map_err(|e| format!("wersja „{to}”: {e}"))?;
        let state = self
            .updater
            .state()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "brak current.json".to_owned())?;
        if state.active == to {
            return Ok(());
        }
        if state.previous.as_ref() != Some(&to) {
            return Err(format!(
                "wersja {to} nie jest poprzednią — rollback tylko o jeden krok"
            ));
        }
        self.updater
            .rollback_by("watchdog")
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
