//! Skażenie (taint) na poziomie **sesji** (przegląd bezpieczeństwa #2, P2-07). Przebieg, który
//! zobaczył treść niezaufaną, skaża sesję; każdy kolejny przebieg tej sesji (następna tura czatu,
//! podzadanie, Krytyczka, zadanie schedulera, wznowienie) startuje skażony — także gdy historia
//! z wcześniejszej tury trafia do kontekstu bez bloku delimitacji. Skażenie jest monotoniczne:
//! pierwsze źródło zostaje, kolejne go nie zmieniają, a agentka nie ma drogi do resetu.
//!
//! Reset wyłącznie jawnie przez właściciela gestem nie-głosowym ([`TaintReset::by_owner`] przyjmuje
//! tylko `CommandOrigin::UserText`; głos, agentka i treść niezaufana — odmowa). Broker trzyma
//! własny taint sesji do jej końca (egress nadal pyta), więc przy ledgerze opartym o Brokera reset
//! działa dopiero w nowej sesji. **Decyzja do potwierdzenia przez człowieka** (SPEC `agent-runtime`).

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use core_bus_contract::SessionId;
use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::TaintSource;

/// Odmowa resetu skażenia.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("reset skażenia sesji odrzucony: {0}")]
pub struct TaintResetError(pub String);

/// Potwierdzenie resetu skażenia przez właściciela (tylko gest nie-głosowy w UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintReset {
    _private: (),
}

impl TaintReset {
    /// Potwierdzenie z UI; głos, agentka i treść niezaufana — odmowa.
    pub fn by_owner(origin: &CommandOrigin) -> Result<Self, TaintResetError> {
        match origin {
            CommandOrigin::UserText => Ok(Self { _private: () }),
            CommandOrigin::UserVoice { .. } => Err(TaintResetError(
                "głosem nie — potwierdź w oknie Alfy (kliknięciem albo klawiaturą)".into(),
            )),
            CommandOrigin::Agent | CommandOrigin::UntrustedContent => Err(TaintResetError(
                "reset może zlecić tylko właściciel, nie agentka ani treść z zewnątrz".into(),
            )),
        }
    }
}

/// Rejestr skażenia sesji.
pub trait SessionTaint: Send + Sync {
    /// Pierwsze źródło skażenia sesji (`None` — sesja czysta).
    fn taint(&self, session: &SessionId) -> Option<TaintSource>;
    /// Oznacza sesję jako skażoną (monotonicznie — pierwsze źródło zostaje).
    fn mark(&self, session: &SessionId, source: &TaintSource);
    /// Reset przez właściciela (potwierdzenie nie-głosem).
    fn reset(&self, session: &SessionId, confirmation: &TaintReset) -> Result<(), TaintResetError>;
}

/// Rejestr w pamięci procesu (domyślny w runtime; wspólny dla wszystkich przebiegów runtime).
#[derive(Debug, Default)]
pub struct MemorySessionTaint {
    map: Mutex<BTreeMap<SessionId, TaintSource>>,
}

impl MemorySessionTaint {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<SessionId, TaintSource>> {
        self.map.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl SessionTaint for MemorySessionTaint {
    fn taint(&self, session: &SessionId) -> Option<TaintSource> {
        self.lock().get(session).cloned()
    }

    fn mark(&self, session: &SessionId, source: &TaintSource) {
        self.lock()
            .entry(session.clone())
            .or_insert_with(|| source.clone());
    }

    fn reset(
        &self,
        session: &SessionId,
        _confirmation: &TaintReset,
    ) -> Result<(), TaintResetError> {
        self.lock().remove(session);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use risk_classifier_contract::SttConfidence;

    #[test]
    fn ledger_is_monotonic_and_reset_needs_owner_text() {
        let l = MemorySessionTaint::default();
        let (s1, s2) = (SessionId::new("s1"), SessionId::new("s2"));
        assert_eq!(l.taint(&s1), None);
        l.mark(&s1, &TaintSource::Web);
        l.mark(&s1, &TaintSource::Email);
        assert_eq!(
            l.taint(&s1),
            Some(TaintSource::Web),
            "pierwsze źródło zostaje"
        );
        assert_eq!(l.taint(&s2), None, "inne sesje czyste");
        let voice = CommandOrigin::UserVoice {
            confidence: SttConfidence::from_permille(990),
            speaker_verified: true,
        };
        for origin in [voice, CommandOrigin::Agent, CommandOrigin::UntrustedContent] {
            assert!(TaintReset::by_owner(&origin).is_err(), "{origin:?}");
        }
        let ok = TaintReset::by_owner(&CommandOrigin::UserText).unwrap();
        l.reset(&s1, &ok).unwrap();
        assert_eq!(l.taint(&s1), None);
    }
}
