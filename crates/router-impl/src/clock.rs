//! Zegary Routera: tokio (w testach z `start_paused` — czas wirtualny) i ręczny.

use std::sync::atomic::{AtomicU64, Ordering};

use router_contract::RouterClock;
use tokio::time::Instant;

/// Zegar monotoniczny `tokio::time::Instant` (ms od utworzenia).
#[derive(Debug)]
pub struct TokioClock(Instant);

impl TokioClock {
    /// Zegar od teraz.
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for TokioClock {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterClock for TokioClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Zegar ręczny (testy).
#[derive(Debug, Default)]
pub struct ManualClock(AtomicU64);

impl ManualClock {
    /// Zegar od 0 ms.
    pub fn new() -> Self {
        Self::default()
    }

    /// Przesuwa zegar.
    pub fn advance_ms(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl RouterClock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
