//! Wirtualny zegar do deterministycznych testów.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, TimeZone, Utc};

/// Zegar, który płynie tylko na żądanie testu. Klonowanie dzieli stan.
#[derive(Debug, Clone)]
pub struct VirtualClock {
    now: Arc<Mutex<DateTime<Utc>>>,
}

impl VirtualClock {
    /// Zegar ustawiony na podany czas.
    pub fn starting_at(start: DateTime<Utc>) -> Self {
        Self {
            now: Arc::new(Mutex::new(start)),
        }
    }

    /// Bieżący czas wirtualny.
    pub fn now(&self) -> DateTime<Utc> {
        self.now
            .lock()
            .map_or_else(|poisoned| *poisoned.into_inner(), |guard| *guard)
    }

    /// Przesuwa zegar do przodu i zwraca nowy czas.
    pub fn advance(&self, by: Duration) -> DateTime<Utc> {
        let mut guard = self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard += by;
        *guard
    }

    /// Ustawia zegar na konkretny czas (także wstecz — do testów odporności).
    pub fn set(&self, to: DateTime<Utc>) {
        let mut guard = self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = to;
    }
}

impl Default for VirtualClock {
    /// Stały punkt startowy: 2026-01-01T00:00:00Z.
    fn default() -> Self {
        Self::starting_at(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
                .single()
                .unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_is_shared_between_clones() {
        let clock = VirtualClock::default();
        let other = clock.clone();
        clock.advance(Duration::seconds(5));
        assert_eq!(other.now(), clock.now());
        assert_eq!(other.now().timestamp(), 1_767_225_605);
    }
}
