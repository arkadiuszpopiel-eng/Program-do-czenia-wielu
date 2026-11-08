//! Atrapy portów: dziennik w pamięci, źródło kursu ze skryptu, zegar sterowany.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use cost_meter_contract::{
    CostClock, CostError, CostRecord, FxError, FxQuote, FxSource, LedgerStore, LoadedLedger,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Dziennik w pamięci z wstrzykiwaniem błędu zapisu.
#[derive(Debug, Default)]
pub struct MemoryLedgerStore {
    records: Mutex<Vec<CostRecord>>,
    fail_next: Mutex<bool>,
}

impl MemoryLedgerStore {
    /// Pusty dziennik.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dziennik z rekordami (symulacja restartu).
    pub fn with_records(records: Vec<CostRecord>) -> Self {
        Self {
            records: Mutex::new(records),
            fail_next: Mutex::new(false),
        }
    }

    /// Następny zapis się nie uda.
    pub fn fail_next_append(&self) {
        *lock(&self.fail_next) = true;
    }

    /// Zapisane rekordy.
    pub fn records(&self) -> Vec<CostRecord> {
        lock(&self.records).clone()
    }
}

impl LedgerStore for MemoryLedgerStore {
    fn append(&self, record: &CostRecord) -> Result<(), CostError> {
        if std::mem::take(&mut *lock(&self.fail_next)) {
            return Err(CostError::Storage("wstrzyknięty błąd zapisu".into()));
        }
        lock(&self.records).push(record.clone());
        Ok(())
    }

    fn load(&self) -> Result<LoadedLedger, CostError> {
        Ok(LoadedLedger {
            records: self.records(),
            skipped_lines: 0,
        })
    }
}

/// Źródło kursu: kolejka wyników, potem wynik domyślny; liczy wywołania.
#[derive(Debug)]
pub struct FakeFxSource {
    queue: Mutex<VecDeque<Result<FxQuote, FxError>>>,
    default: Mutex<Result<FxQuote, FxError>>,
    calls: Mutex<usize>,
}

impl FakeFxSource {
    /// Zawsze zwraca notowanie.
    pub fn fixed(quote: FxQuote) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            default: Mutex::new(Ok(quote)),
            calls: Mutex::new(0),
        }
    }

    /// Zawsze błąd sieci (brak NBP).
    pub fn offline() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            default: Mutex::new(Err(FxError::Network("brak sieci (atrapa)".into()))),
            calls: Mutex::new(0),
        }
    }

    /// Następne wywołania zwrócą te wyniki (FIFO).
    pub fn push(&self, result: Result<FxQuote, FxError>) {
        lock(&self.queue).push_back(result);
    }

    /// Zmienia wynik domyślny.
    pub fn set_default(&self, result: Result<FxQuote, FxError>) {
        *lock(&self.default) = result;
    }

    /// Liczba wywołań.
    pub fn calls(&self) -> usize {
        *lock(&self.calls)
    }
}

#[async_trait]
impl FxSource for FakeFxSource {
    async fn fetch_usd_pln(&self) -> Result<FxQuote, FxError> {
        *lock(&self.calls) += 1;
        let queued = lock(&self.queue).pop_front();
        queued.unwrap_or_else(|| lock(&self.default).clone())
    }
}

/// Zegar sterowany: „teraz” = wskazany dzień, 12:00 UTC.
#[derive(Debug)]
pub struct FixedClock {
    day: Mutex<NaiveDate>,
}

impl FixedClock {
    /// Zegar na dany dzień.
    pub fn new(day: NaiveDate) -> Self {
        Self {
            day: Mutex::new(day),
        }
    }

    /// Ustawia dzień.
    pub fn set_day(&self, day: NaiveDate) {
        *lock(&self.day) = day;
    }

    /// Przesuwa o dni.
    pub fn advance_days(&self, days: i64) {
        let mut d = lock(&self.day);
        *d += chrono::Duration::days(days);
    }
}

impl CostClock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        let day = *lock(&self.day);
        day.and_hms_opt(12, 0, 0)
            .map(|dt| Utc.from_utc_datetime(&dt))
            .unwrap_or_default()
    }

    fn today(&self) -> NaiveDate {
        *lock(&self.day)
    }
}
