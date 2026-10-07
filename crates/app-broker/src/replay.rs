//! Odtwarzanie stanu bezpieczeństwa po ponownym połączeniu (przegląd bezpieczeństwa #3, SR3-03).
//!
//! Broker trzyma skażenie sesji i poziomy autonomii wyłącznie w pamięci, a proces Brokera bywa
//! uruchamiany ponownie (tryb przenośny — nadzór [`crate::supervise`] po awarii; usługa — SCM).
//! Nowy Broker nie wiedziałby, że sesja widziała niezaufaną treść (reguła `TaintedEgress`
//! obowiązuje „także na L4”) ani że właściciel obniżył poziom autonomii („panika” L0 wracałaby do
//! domyślnego L3) — to otwarcie zabezpieczeń (fail-open) zamiast bezpiecznego stanu.
//!
//! Dziennik zapamiętuje wyłącznie to, co **zawęża**: skażenie sesji (monotoniczne) i obniżenia
//! poziomu przez właściciela poniżej domyślnego L3. Łącze odtwarza go na każdym nowym połączeniu
//! **przed** udostępnieniem połączenia wołającym (żadna decyzja nie wyprzedzi odtworzenia).
//! Podniesienia nie są odtwarzane (wymagają zgody w Broker-UI); obniżenie odtworzone po
//! późniejszym podniesieniu daje stan ostrzejszy niż przed restartem — kierunek bezpieczny.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use core_bus_contract::SessionId;
use safety_broker_contract::ipc::{Request, UserChannel};
use safety_broker_contract::{AutonomyLevel, AutonomyTarget, TaintSource};

/// Najwięcej zapamiętanych sesji i celów obniżeń (sesje żyją krócej niż aplikacja; po
/// przekroczeniu — błąd w dzienniku, wpis i tak trafia do Brokera, ale nie do odtworzenia).
pub const MAX_ENTRIES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Lowered {
    level: AutonomyLevel,
    until_ms: Option<u64>,
    via: UserChannel,
}

#[derive(Debug, Default)]
struct Entries {
    taint: BTreeMap<SessionId, Vec<TaintSource>>,
    lowered: BTreeMap<AutonomyTarget, Lowered>,
}

/// Dziennik stanu zawężającego do odtworzenia w nowym Brokerze.
#[derive(Debug, Default)]
pub struct Journal {
    entries: Mutex<Entries>,
}

impl Journal {
    fn lock(&self) -> MutexGuard<'_, Entries> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Sesja skażona (Broker przyjął zgłoszenie albo sam ją oznaczył).
    pub fn taint(&self, session: &SessionId, source: &TaintSource) {
        let mut e = self.lock();
        if !e.taint.contains_key(session) && e.taint.len() >= MAX_ENTRIES {
            tracing::error!(
                sesja = %session,
                "dziennik skażenia pełny — skażenie tej sesji nie przetrwa restartu Brokera"
            );
            return;
        }
        let sources = e.taint.entry(session.clone()).or_default();
        if !sources.contains(source) {
            sources.push(source.clone());
        }
    }

    /// Obniżenie poziomu przez właściciela (zastosowane przez Brokera bez zgody). Pamiętane tylko
    /// poniżej domyślnego L3 — wyższe poziomy i tak nie przetrwają restartu (bezpiecznie).
    pub fn lowered(
        &self,
        target: &AutonomyTarget,
        level: AutonomyLevel,
        until_ms: Option<u64>,
        via: UserChannel,
    ) {
        if level >= AutonomyLevel::L3 {
            return;
        }
        let mut e = self.lock();
        if !e.lowered.contains_key(target) && e.lowered.len() >= MAX_ENTRIES {
            tracing::error!("dziennik obniżeń poziomu pełny — obniżenie nie przetrwa restartu");
            return;
        }
        e.lowered.insert(
            target.clone(),
            Lowered {
                level,
                until_ms,
                via,
            },
        );
    }

    /// Żądania odtwarzające stan (skażenia, potem obniżenia).
    pub fn requests(&self) -> Vec<Request> {
        let e = self.lock();
        let taint = e.taint.iter().flat_map(|(session, sources)| {
            sources.iter().map(|source| Request::ReportUntrusted {
                session: session.clone(),
                source: source.clone(),
            })
        });
        let lowered = e
            .lowered
            .iter()
            .map(|(target, l)| Request::RequestAutonomy {
                target: target.clone(),
                level: l.level,
                until_ms: l.until_ms,
                via: l.via,
            });
        taint.chain(lowered).collect()
    }

    /// Liczba zapamiętanych wpisów (sesje skażone + cele obniżeń).
    pub fn len(&self) -> usize {
        let e = self.lock();
        e.taint.len() + e.lowered.len()
    }

    /// Czy dziennik jest pusty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_only_narrowing_state_without_duplicates() {
        let j = Journal::default();
        let s = SessionId::new("s1");
        j.taint(&s, &TaintSource::Web);
        j.taint(&s, &TaintSource::Web);
        j.taint(&s, &TaintSource::File);
        let target = AutonomyTarget::Session { session: s.clone() };
        j.lowered(&target, AutonomyLevel::L4, None, UserChannel::UserInterface);
        j.lowered(&target, AutonomyLevel::L3, None, UserChannel::UserInterface);
        assert_eq!(j.requests().len(), 2, "dwa źródła skażenia, bez L3/L4");
        j.lowered(&target, AutonomyLevel::L1, None, UserChannel::UserInterface);
        j.lowered(&target, AutonomyLevel::L0, Some(9), UserChannel::UserVoice);
        let requests = j.requests();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests.last(),
            Some(&Request::RequestAutonomy {
                target,
                level: AutonomyLevel::L0,
                until_ms: Some(9),
                via: UserChannel::UserVoice,
            })
        );
        assert_eq!(j.len(), 2);
        assert!(!j.is_empty());
    }
}
