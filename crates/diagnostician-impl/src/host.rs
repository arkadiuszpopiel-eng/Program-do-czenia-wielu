//! Otoczenie usługi: zegar, kanał zdarzeń na magistralę, ostatnie zdarzenia, dziennik w pliku.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::{Event, Level};
use diagnostician_contract::{DiagHost, JournalEntry, diag_event};
use tokio::sync::mpsc::UnboundedSender;
use watchdog_contract::Clock;

use crate::journal_file::FileJournal;

/// Ile ostatnich zdarzeń trzyma usługa.
pub const RECENT_EVENTS: usize = 256;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Otoczenie usługi Diagnosty.
pub struct ServiceHost {
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) events: Mutex<Option<UnboundedSender<Vec<Event>>>>,
    pub(crate) recent: Mutex<VecDeque<Event>>,
    pub(crate) journal: Option<FileJournal>,
}

impl ServiceHost {
    fn push(&self, events: Vec<Event>) {
        {
            let mut recent = lock(&self.recent);
            recent.extend(events.iter().cloned());
            while recent.len() > RECENT_EVENTS {
                recent.pop_front();
            }
        }
        if let Some(tx) = lock(&self.events).as_ref() {
            let _ = tx.send(events);
        }
    }
}

impl DiagHost for ServiceHost {
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn emit(&self, events: Vec<Event>) {
        self.push(events);
    }

    fn append(&self, entry: &JournalEntry) {
        if let Some(journal) = &self.journal
            && let Err(e) = journal.append(entry)
        {
            let payload = serde_json::json!({ "error": e.to_string(), "seq": entry.seq });
            self.push(vec![diag_event(
                "diagnostician.journal_failed",
                Level::Error,
                payload,
            )]);
        }
    }
}
