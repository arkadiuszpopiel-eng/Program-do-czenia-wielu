//! Audyt Brokera: synchroniczny ujście [`AuditSink`] (kolejność zapisów = kolejność decyzji),
//! writer pliku z łańcuchem SHA-256 i kotwicą, wersja w pamięci dla testów i atrapy.

mod anchor;
mod canonical;
mod chain;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use core_bus_contract::Event;
use core_log_contract::{AuditRecordRef, LogError};

pub use anchor::{FileAnchorStore, MemoryAnchorStore};
pub use canonical::{canonical_json, sha256_hex};
pub use chain::{
    BROKER_WRITER, BrokerAuditWriter, ChainError, ChainSummary, verify_bytes, verify_file,
};

/// Synchroniczne ujście Audytu używane przez silnik Brokera.
pub trait AuditSink: Send + Sync {
    /// Dopisuje rekord; błąd = Broker nie wydaje tokenu (fail-closed).
    fn record(&self, event: &Event) -> Result<AuditRecordRef, LogError>;
}

/// Audyt w pamięci z łańcuchem hashy (testy, atrapa) i sterowaną awarią.
#[derive(Debug, Default)]
pub struct MemoryAudit {
    events: Mutex<Vec<(Event, String)>>,
    fail: AtomicBool,
}

impl MemoryAudit {
    /// Zapisane zdarzenia.
    pub fn events(&self) -> Vec<Event> {
        self.lock().iter().map(|(e, _)| e.clone()).collect()
    }

    /// Nazwy zapisanych zdarzeń (rodzaje).
    pub fn names(&self) -> Vec<String> {
        self.lock()
            .iter()
            .map(|(e, _)| e.kind.as_str().to_owned())
            .collect()
    }

    /// Przełącza awarię zapisu (test fail-closed).
    pub fn set_failing(&self, failing: bool) {
        self.fail.store(failing, Ordering::SeqCst);
    }

    fn lock(&self) -> MutexGuard<'_, Vec<(Event, String)>> {
        self.events.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl AuditSink for MemoryAudit {
    fn record(&self, event: &Event) -> Result<AuditRecordRef, LogError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(LogError::Io("Audyt niedostępny (test)".into()));
        }
        let mut st = self.lock();
        let mut e = event.clone();
        e.prev_hash = st.last().map(|(_, h)| h.clone());
        let body = serde_json::to_value(&e).unwrap_or_default();
        let hash = sha256_hex(canonical_json(&body).as_bytes());
        let seq = st.len() as u64;
        st.push((e, hash.clone()));
        Ok(AuditRecordRef { seq, hash })
    }
}
