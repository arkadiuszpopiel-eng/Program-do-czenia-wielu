//! Bezpiecznik silnika rozpoznawania mowy (reguła z Delty AI): po kilku awariach z rzędu silnik jest
//! odstawiany, a rozpoznaje silnik zapasowy; po przerwie wpuszczana jest jedna próba. Czas podaje
//! wywołujący, więc logika ma testy bez zegara.

use std::time::Duration;

/// Stan bezpiecznika silnika.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    /// Silnik działa.
    Closed,
    /// Silnik odstawiony — rozpoznaje silnik zapasowy.
    Open,
    /// Po przerwie: wpuszczona jedna próba.
    HalfOpen,
}

/// Bezpiecznik: po `failures_to_open` awariach z rzędu odstawia silnik na `recovery`, potem wpuszcza
/// dokładnie jedną próbę. Sukces zamyka i zeruje licznik, porażka próby otwiera ponownie.
#[derive(Debug, Clone)]
pub struct Breaker {
    failures_to_open: u32,
    recovery: Duration,
    state: BreakerState,
    failures: u32,
    opened_at: Duration,
    opened_count: u32,
}

impl Breaker {
    /// Bezpiecznik z progami sprowadzonymi do zakresów (1–20 awarii, 5–3600 s przerwy).
    pub fn new(failures_to_open: u32, recovery: Duration) -> Self {
        Self {
            failures_to_open: failures_to_open.clamp(1, 20),
            recovery: recovery.clamp(Duration::from_secs(5), Duration::from_secs(3600)),
            state: BreakerState::Closed,
            failures: 0,
            opened_at: Duration::ZERO,
            opened_count: 0,
        }
    }

    /// Czy wolno teraz użyć silnika (`now` = czas od startu programu). W stanie otwartym po
    /// upływie przerwy przepuszcza jedną próbę; kolejne żądania czekają na jej wynik.
    pub fn allow(&mut self, now: Duration) -> bool {
        match self.state {
            BreakerState::Closed => true,
            BreakerState::HalfOpen => false,
            BreakerState::Open => {
                if now.saturating_sub(self.opened_at) >= self.recovery {
                    self.state = BreakerState::HalfOpen;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Wynik użycia silnika.
    pub fn record(&mut self, ok: bool, now: Duration) {
        if ok {
            self.state = BreakerState::Closed;
            self.failures = 0;
            return;
        }
        self.failures += 1;
        if self.state == BreakerState::HalfOpen || self.failures >= self.failures_to_open {
            self.state = BreakerState::Open;
            self.opened_at = now;
            self.opened_count += 1;
        }
    }

    /// Ręczne zamknięcie („Zamknij bezpiecznik” w oknie).
    pub fn reset(&mut self) {
        self.state = BreakerState::Closed;
        self.failures = 0;
    }

    /// Stan, awarie z rzędu, próg i liczba otwarć — do pokazania w oknie.
    pub fn snapshot(&self) -> (BreakerState, u32, u32, u32) {
        (
            self.state,
            self.failures,
            self.failures_to_open,
            self.opened_count,
        )
    }

    /// Ile zostało do następnej próby (stan otwarty); `None` w pozostałych stanach.
    pub fn retry_in(&self, now: Duration) -> Option<Duration> {
        (self.state == BreakerState::Open).then(|| {
            self.recovery
                .saturating_sub(now.saturating_sub(self.opened_at))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaker_opens_after_three_failures_and_allows_one_probe() {
        let s = Duration::from_secs;
        let mut b = Breaker::new(3, s(30));
        assert!(b.allow(s(0)));
        b.record(false, s(1));
        b.record(false, s(2));
        assert_eq!(b.snapshot().0, BreakerState::Closed);
        b.record(false, s(3));
        assert_eq!(b.snapshot(), (BreakerState::Open, 3, 3, 1));
        assert!(!b.allow(s(20)), "przerwa trwa");
        assert_eq!(b.retry_in(s(20)), Some(s(13)));
        assert!(b.allow(s(34)), "jedna próba po 31 s");
        assert!(!b.allow(s(34)), "druga równoległa odmowa");
        b.record(false, s(35));
        assert_eq!(
            b.snapshot().0,
            BreakerState::Open,
            "nieudana próba otwiera ponownie"
        );
        assert_eq!(b.snapshot().3, 2);
        assert!(b.allow(s(66)));
        b.record(true, s(67));
        assert_eq!(b.snapshot(), (BreakerState::Closed, 0, 3, 2));
        assert_eq!(b.retry_in(s(67)), None);
    }

    #[test]
    fn breaker_limits_are_clamped_and_reset_closes() {
        let mut b = Breaker::new(0, Duration::from_secs(1));
        b.record(false, Duration::ZERO);
        assert_eq!(
            b.snapshot(),
            (BreakerState::Open, 1, 1, 1),
            "próg co najmniej 1"
        );
        assert!(!b.allow(Duration::from_secs(4)), "przerwa co najmniej 5 s");
        b.reset();
        assert!(b.allow(Duration::from_secs(4)));
        assert_eq!(
            Breaker::new(99, Duration::from_secs(99_999)).snapshot().2,
            20
        );
    }
}
