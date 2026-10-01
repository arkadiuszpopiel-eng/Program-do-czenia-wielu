//! Otoczenie usługi: zegar, kanał zdarzeń, trwała kolejka propozycji.

use std::collections::VecDeque;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use core_bus_contract::{Event, Level};
use evals_contract::Clock;
use improver_contract::{ImproverHost, Proposal, improver_event};
use tokio::sync::mpsc::UnboundedSender;

/// Zapis atomowy (plik tymczasowy + `rename`).
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".tmp-{}", std::process::id()));
    let tmp = path.with_file_name(name);
    let written = fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()
    });
    match written.and_then(|()| fs::rename(&tmp, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Wczytuje kolejkę propozycji (brak pliku = pusta; uszkodzony = błąd).
pub fn load_proposals(path: &Path) -> Result<Vec<Proposal>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Ile ostatnich zdarzeń trzyma usługa (panel „Zdrowie systemu”, diagnostyka).
pub const RECENT_EVENTS: usize = 256;

/// Otoczenie usługi.
pub struct ServiceHost {
    pub(crate) recent: Mutex<VecDeque<Event>>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) events: Mutex<Option<UnboundedSender<Vec<Event>>>>,
    pub(crate) state_path: Option<PathBuf>,
}

impl ImproverHost for ServiceHost {
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn emit(&self, events: Vec<Event>) {
        {
            let mut recent = self.recent.lock().unwrap_or_else(|p| p.into_inner());
            recent.extend(events.iter().cloned());
            while recent.len() > RECENT_EVENTS {
                recent.pop_front();
            }
        }
        // Kanał nieograniczony; moduł zatrzymany (brak nadawcy) — na magistralę nie trafiają.
        let guard = self.events.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(events);
        }
    }

    fn persist(&self, proposals: &[Proposal]) {
        let Some(path) = &self.state_path else { return };
        let saved = serde_json::to_vec_pretty(proposals)
            .map_err(|e| e.to_string())
            .and_then(|bytes| write_atomic(path, &bytes).map_err(|e| e.to_string()));
        if let Err(error) = saved {
            self.emit(vec![improver_event(
                "improver.persist_failed",
                Level::Error,
                serde_json::json!({ "error": error }),
            )]);
        }
    }
}
