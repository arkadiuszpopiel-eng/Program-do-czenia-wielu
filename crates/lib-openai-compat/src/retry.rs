//! Wykładniczy backoff z jitterem (bez zewnętrznego RNG).

use std::hash::{BuildHasher, Hasher};
use std::time::Duration;

use crate::config::RetryPolicy;

/// Losowe `u64` z `RandomState` (ziarno systemowe) — wystarczające do rozproszenia ponowień.
fn random_u64() -> u64 {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.finish()
}

/// „Równy jitter": losowo w `[d/2, d]`.
fn jitter(d: Duration) -> Duration {
    let half = d / 2;
    let span = u64::try_from((d - half).as_nanos()).unwrap_or(u64::MAX);
    half + Duration::from_nanos(random_u64() % span.saturating_add(1))
}

impl RetryPolicy {
    /// Opóźnienie przed ponowieniem nr `attempt` (od 1). `retry_after` dostawcy wygrywa, ale tylko
    /// do `max_retry_after` — dłuższy → `None` (nie czekamy, Router przełącza).
    pub fn delay(&self, attempt: u32, retry_after: Option<Duration>) -> Option<Duration> {
        if let Some(d) = retry_after {
            return (d <= self.max_retry_after).then_some(d);
        }
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        Some(jitter(
            self.base_delay.saturating_mul(factor).min(self.max_delay),
        ))
    }

    /// Czy wolno ponowić: numer próby w limicie i opóźnienie mieści się w budżecie.
    pub fn allows(&self, attempt: u32, elapsed: Duration, delay: Duration) -> bool {
        attempt <= self.max_retries && elapsed + delay <= self.budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_with_jitter_and_caps() {
        let p = RetryPolicy::default();
        for attempt in 1..=6 {
            let d = p.delay(attempt, None).unwrap();
            let full = p
                .base_delay
                .saturating_mul(1 << (attempt - 1))
                .min(p.max_delay);
            assert!(d >= full / 2 && d <= full, "{attempt}: {d:?} vs {full:?}");
        }
        assert_eq!(
            p.delay(1, Some(Duration::from_millis(300))),
            Some(Duration::from_millis(300))
        );
        assert_eq!(p.delay(1, Some(Duration::from_secs(30))), None);
        assert!(p.allows(1, Duration::ZERO, Duration::from_millis(100)));
        assert!(!p.allows(3, Duration::ZERO, Duration::from_millis(1)));
        assert!(!p.allows(1, Duration::from_millis(1_450), Duration::from_millis(100)));
        assert_eq!(jitter(Duration::ZERO), Duration::ZERO);
    }
}
