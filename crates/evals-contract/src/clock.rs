//! Zegar w milisekundach (budżet zapytań do holdoutu, znaczniki raportów).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Źródło czasu (ms od epoki UNIX).
pub trait Clock: Send + Sync {
    /// Bieżący czas w ms.
    fn now_ms(&self) -> u64;
}

/// Zegar systemowy.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }
}

/// Wirtualny zegar sterowany przez test.
#[derive(Debug, Default)]
pub struct ManualClock {
    now: AtomicU64,
}

impl ManualClock {
    /// Zegar ustawiony na `start_ms`.
    pub fn new(start_ms: u64) -> Self {
        Self {
            now: AtomicU64::new(start_ms),
        }
    }

    /// Przesuwa czas o `ms`.
    pub fn advance(&self, ms: u64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}
