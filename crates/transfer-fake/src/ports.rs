//! Porty atrapy: dokumenty w pamięci, wirtualny zegar, deterministyczne identyfikatory kopii
//! i skryptowane awarie (licznik operacji zapisu — „przerwanie w połowie”).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Duration, TimeZone, Utc};
use sessions_contract::SessionId;
use transfer_contract::{Clock, DocumentStore, IdSource, TransferError, validate_entry_path};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Budżet operacji zapisu współdzielony przez porty: po wyczerpaniu każda operacja zapisu zawodzi
/// (symulacja awarii/zamknięcia programu w połowie importu). `None` = bez limitu.
#[derive(Debug, Clone, Default)]
pub struct FailureBudget(Arc<Mutex<Option<u64>>>);

impl FailureBudget {
    /// Bez limitu.
    pub fn unlimited() -> Self {
        Self::default()
    }

    /// Pozwala na `n` kolejnych zapisów, potem błędy.
    pub fn allow(&self, n: u64) {
        *lock(&self.0) = Some(n);
    }

    /// Znosi limit („restart programu”).
    pub fn reset(&self) {
        *lock(&self.0) = None;
    }

    /// Zużywa jedną operację; błąd, gdy budżet wyczerpany.
    pub fn spend(&self, what: &str) -> Result<(), String> {
        let mut left = lock(&self.0);
        match left.as_mut() {
            None => Ok(()),
            Some(0) => Err(format!("symulowana awaria przy: {what}")),
            Some(n) => {
                *n -= 1;
                Ok(())
            }
        }
    }
}

/// Magazyn dokumentów w pamięci (atomowy zapis z definicji).
#[derive(Debug, Default)]
pub struct MemoryDocumentStore {
    docs: Mutex<BTreeMap<String, Vec<u8>>>,
    budget: FailureBudget,
}

impl MemoryDocumentStore {
    /// Pusty magazyn.
    pub fn new() -> Self {
        Self::default()
    }

    /// Magazyn ze wspólnym budżetem awarii.
    pub fn with_budget(budget: FailureBudget) -> Self {
        Self {
            docs: Mutex::new(BTreeMap::new()),
            budget,
        }
    }

    fn check(&self, what: &str) -> Result<(), TransferError> {
        self.budget
            .spend(what)
            .map_err(|e| TransferError::port("dokumenty", e))
    }
}

impl DocumentStore for MemoryDocumentStore {
    fn list(&self) -> Result<Vec<String>, TransferError> {
        Ok(lock(&self.docs).keys().cloned().collect())
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, TransferError> {
        Ok(lock(&self.docs).get(name).cloned())
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), TransferError> {
        validate_entry_path(name).map_err(|reason| TransferError::UnsafePath {
            path: name.to_owned(),
            reason,
        })?;
        self.check(name)?;
        lock(&self.docs).insert(name.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<bool, TransferError> {
        self.check(name)?;
        Ok(lock(&self.docs).remove(name).is_some())
    }
}

/// Wirtualny zegar: od 2026-01-01, każde odczytanie przesuwa czas o 1 s (deterministyczne
/// i unikalne znaczniki nazw kopii/snapshotów).
#[derive(Debug)]
pub struct VirtualClock {
    now: Mutex<DateTime<Utc>>,
}

impl Default for VirtualClock {
    fn default() -> Self {
        let start = Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .single()
            .unwrap_or_default();
        Self {
            now: Mutex::new(start),
        }
    }
}

impl VirtualClock {
    /// Przesuwa zegar.
    pub fn advance(&self, by: Duration) {
        *lock(&self.now) += by;
    }
}

impl Clock for VirtualClock {
    fn now(&self) -> DateTime<Utc> {
        let mut now = lock(&self.now);
        let current = *now;
        *now += Duration::seconds(1);
        current
    }
}

/// Identyfikatory kopii `copy-0001`, `copy-0002`…
#[derive(Debug, Default)]
pub struct SeqIds(AtomicU64);

impl IdSource for SeqIds {
    fn new_session_id(&self) -> SessionId {
        let n = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        SessionId::new(format!("copy-{n:04}"))
    }
}
