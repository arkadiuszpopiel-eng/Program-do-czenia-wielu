//! Porty silnika pamięci F7: zegar, identyfikatory, prywatność sesji, zdarzenia.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, TimeZone, Utc};
use core_bus_contract::SessionId;

/// Zegar (wirtualny w atrapie).
pub trait MemoryClock: Send + Sync {
    /// Teraz (UTC).
    fn now(&self) -> DateTime<Utc>;
}

/// Zegar systemowy.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl MemoryClock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Zegar wirtualny: start 2026-01-01 00:00 UTC, każde `now()` przesuwa czas o 1 s (deterministyczne
/// porządki); [`VirtualClock::advance`] przesuwa o dowolny czas (testy TTL i retencji).
#[derive(Debug, Default)]
pub struct VirtualClock {
    ticks: AtomicU64,
    offset_secs: AtomicU64,
}

impl VirtualClock {
    /// Nowy zegar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Przesuwa czas o `secs` sekund.
    pub fn advance(&self, secs: u64) {
        self.offset_secs.fetch_add(secs, Ordering::SeqCst);
    }
}

impl MemoryClock for VirtualClock {
    fn now(&self) -> DateTime<Utc> {
        let tick = self.ticks.fetch_add(1, Ordering::SeqCst) + 1;
        let secs = tick.saturating_add(self.offset_secs.load(Ordering::SeqCst));
        let base = Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .single()
            .unwrap_or_default();
        base + chrono::Duration::seconds(i64::try_from(secs).unwrap_or(i64::MAX / 2))
    }
}

/// Źródło identyfikatorów wpisów i zmian.
pub trait IdSource: Send + Sync {
    /// Nowy unikalny identyfikator (`[A-Za-z0-9_-]`).
    fn next_id(&self, prefix: &str) -> String;
}

/// Kolejne identyfikatory `<prefiks>-0001`… (atrapa, testy).
#[derive(Debug, Default)]
pub struct SeqIds {
    next: AtomicU64,
}

impl SeqIds {
    /// Nowe źródło.
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdSource for SeqIds {
    fn next_id(&self, prefix: &str) -> String {
        let n = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        format!("{prefix}-{n:04}")
    }
}

/// Prywatność sesji (PLAN §11: tag prywatności). Sesja prywatna (`private`, `local_only`) **nigdy**
/// nie zasila zakresów szerszych niż ona sama.
pub trait PrivacyOracle: Send + Sync {
    /// Czy sesja jest prywatna. Sesja nieznana → `true` (bezpieczniej: brak awansu).
    fn is_private(&self, session: &SessionId) -> bool;
}

/// Prywatność sesji w pamięci (testy, atrapa; produkcyjnie — katalog `sessions`).
#[derive(Debug, Default)]
pub struct PrivateSessions {
    private: Mutex<Vec<SessionId>>,
    public: Mutex<Vec<SessionId>>,
    strict: bool,
}

impl PrivateSessions {
    /// Wszystkie sesje publiczne poza oznaczonymi [`PrivateSessions::mark_private`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Tryb ścisły: sesja nieoznaczona przez [`PrivateSessions::mark_public`] jest prywatna.
    pub fn strict() -> Self {
        Self {
            strict: true,
            ..Self::default()
        }
    }

    /// Oznacza sesję jako prywatną.
    pub fn mark_private(&self, session: SessionId) {
        self.private
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(session);
    }

    /// Oznacza sesję jako publiczną (tryb ścisły).
    pub fn mark_public(&self, session: SessionId) {
        self.public
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(session);
    }
}

impl PrivacyOracle for PrivateSessions {
    fn is_private(&self, session: &SessionId) -> bool {
        let lock = |m: &Mutex<Vec<SessionId>>| {
            m.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(session)
        };
        lock(&self.private) || (self.strict && !lock(&self.public))
    }
}

/// Odbiorca zdarzeń `memory.*` (ładunki wyłącznie z identyfikatorami i licznikami).
pub trait EventSink: Send + Sync {
    /// Publikuje zdarzenie (bez blokowania).
    fn emit(&self, kind: &str, session: Option<&SessionId>, payload: serde_json::Value);
}

/// Odbiorca, który niczego nie publikuje.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoEvents;

impl EventSink for NoEvents {
    fn emit(&self, _kind: &str, _session: Option<&SessionId>, _payload: serde_json::Value) {}
}

/// Odbiorca zapisujący zdarzenia (testy: brak treści w ładunkach).
#[derive(Debug, Default)]
pub struct RecordingEvents {
    events: Mutex<Vec<(String, serde_json::Value)>>,
}

impl RecordingEvents {
    /// Nowy odbiorca.
    pub fn new() -> Self {
        Self::default()
    }

    /// Zapisane zdarzenia `(rodzaj, ładunek)`.
    pub fn events(&self) -> Vec<(String, serde_json::Value)> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl EventSink for RecordingEvents {
    fn emit(&self, kind: &str, _session: Option<&SessionId>, payload: serde_json::Value) {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((kind.to_owned(), payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_clock_and_ids_are_deterministic() {
        let c = VirtualClock::new();
        let a = c.now();
        let b = c.now();
        assert!(b > a);
        c.advance(3600);
        assert!(c.now() - b >= chrono::Duration::seconds(3600));
        let ids = SeqIds::new();
        assert_eq!(ids.next_id("mem"), "mem-0001");
        assert_eq!(ids.next_id("chg"), "chg-0002");
        let p = PrivateSessions::new();
        p.mark_private(SessionId::new("P"));
        assert!(p.is_private(&SessionId::new("P")) && !p.is_private(&SessionId::new("A")));
        let ev = RecordingEvents::new();
        ev.emit("x", None, serde_json::json!({}));
        assert_eq!(ev.events().len(), 1);
    }
}
