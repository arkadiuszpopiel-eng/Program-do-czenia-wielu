//! Suma przedziałów oczekiwania, także przy nakładających się prośbach.
use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;
use tokio::time::Instant;
#[derive(Default)]
struct State {
    active: BTreeSet<String>,
    since: Option<Instant>,
    total: Duration,
}
/// Zegar oczekiwania, którego nie zaliczamy do czasu aktywnej pracy.
#[derive(Default)]
pub struct ApprovalClock(Mutex<State>);
impl ApprovalClock {
    /// Początek oczekiwania na tę prośbę.
    pub fn begin(&self, id: String) {
        let mut s = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if s.active.insert(id) && s.active.len() == 1 {
            s.since = Some(Instant::now());
        }
    }
    /// Koniec oczekiwania, niezależnie od decyzji.
    pub fn end(&self, id: &str) {
        let mut s = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if s.active.remove(id)
            && s.active.is_empty()
            && let Some(start) = s.since.take()
        {
            s.total += start.elapsed();
        }
    }
    /// Łącznie przebyte oczekiwanie, z bieżącym otwartym przedziałem.
    pub fn waited_ms(&self) -> u64 {
        let s = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        u64::try_from((s.total + s.since.map_or(Duration::ZERO, |i| i.elapsed())).as_millis())
            .unwrap_or(u64::MAX)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn overlapping_requests_are_counted_once() {
        let c = ApprovalClock::default();
        c.begin("a".into());
        tokio::time::sleep(Duration::from_secs(2)).await;
        c.begin("b".into());
        tokio::time::sleep(Duration::from_secs(2)).await;
        c.end("a");
        tokio::time::sleep(Duration::from_secs(2)).await;
        c.end("b");
        assert_eq!(c.waited_ms(), 6000);
    }
}
