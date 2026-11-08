//! Zegar monotoniczny potoku (gdy `app-core` nie poda zegara urządzenia audio).

use std::time::Instant;

use voice_audio_contract::{MediaClock, MediaTime};

/// Zegar monotoniczny od chwili utworzenia (`Instant`).
#[derive(Debug, Clone, Copy)]
pub struct MonotonicClock {
    origin: Instant,
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MonotonicClock {
    /// Zegar od teraz.
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl MediaClock for MonotonicClock {
    fn now(&self) -> MediaTime {
        MediaTime(u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_monotonic() {
        let c = MonotonicClock::default();
        let a = c.now();
        assert!(c.now() >= a);
    }
}
