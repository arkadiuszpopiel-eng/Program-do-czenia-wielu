//! Zegar w milisekundach: systemowy i wirtualny (testy deterministyczne, PLAN §4.5).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Źródło czasu (ms od epoki UNIX). Watchdog i Broker nie czytają zegara systemowego wprost.
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

    /// Ustawia czas.
    pub fn set(&self, ms: u64) {
        self.now.store(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_moves_only_when_told() {
        let c = ManualClock::new(10);
        assert_eq!(c.now_ms(), 10);
        c.advance(5);
        assert_eq!(c.now_ms(), 15);
        c.set(1);
        assert_eq!(c.now_ms(), 1);
        assert!(SystemClock.now_ms() > 1_600_000_000_000);
    }
}
