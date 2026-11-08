//! Cofanie zapisu zmiennych użytkownika: krok pamięta poprzednią i zapisaną wartość (tylko
//! w pamięci procesu — nigdy w wyniku, zdarzeniach ani logach); cofnięcie przywraca poprzednią
//! wartość wyłącznie, gdy bieżąca jest wciąż tą zapisaną (inaczej konflikt — ktoś zmienił ją
//! później). Dziennik ograniczony (najstarsze kroki wypadają).

use std::collections::VecDeque;

/// Krok cofania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvStep {
    pub(crate) name: String,
    pub(crate) previous: Option<String>,
    pub(crate) written: Option<String>,
}

/// Dziennik kroków.
#[derive(Debug, Default)]
pub(crate) struct UndoLog {
    next: u64,
    steps: VecDeque<(u64, EnvStep)>,
}

impl UndoLog {
    /// Zapamiętuje krok; zwraca jego identyfikator (od 1).
    pub(crate) fn push(&mut self, step: EnvStep, max: usize) -> u64 {
        self.next += 1;
        self.steps.push_back((self.next, step));
        while self.steps.len() > max.max(1) {
            self.steps.pop_front();
        }
        self.next
    }

    /// Krok (bez usuwania).
    pub(crate) fn get(&self, id: u64) -> Option<EnvStep> {
        self.steps
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, s)| s.clone())
    }

    /// Usuwa krok po udanym cofnięciu.
    pub(crate) fn remove(&mut self, id: u64) {
        self.steps.retain(|(i, _)| *i != id);
    }
}

/// Błąd cofnięcia zapisu zmiennej.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvUndoError {
    /// Nieznany (albo już cofnięty / wypadł z dziennika) krok.
    #[error("nieznany krok cofania #{0}")]
    Unknown(u64),
    /// Zmienną zmieniono po zapisie agentki — cofnięcie nadpisałoby cudzą zmianę.
    #[error("zmienna {0} zmieniła się po zapisie — cofnięcie odrzucone")]
    Conflict(String),
    /// Błąd portu.
    #[error("system: {0}")]
    Platform(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_log() {
        let mut log = UndoLog::default();
        let step = |n: &str| EnvStep {
            name: n.into(),
            previous: None,
            written: Some("1".into()),
        };
        let a = log.push(step("A"), 2);
        let b = log.push(step("B"), 2);
        let c = log.push(step("C"), 2);
        assert_eq!((a, b, c), (1, 2, 3));
        assert!(log.get(a).is_none(), "najstarszy wypadł");
        assert_eq!(log.get(c).unwrap().name, "C");
        log.remove(b);
        assert!(log.get(b).is_none());
    }
}
