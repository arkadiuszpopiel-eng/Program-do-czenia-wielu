//! Jednorazowe uchwyty plików dla UI (wzór: ścieżki upuszczenia w `app-files`): ścieżkę zna tylko
//! rdzeń (natywny dialog, lista kopii zapasowych), UI dostaje losowy identyfikator. Uchwyt jest
//! ważny ograniczony czas i zużywa się przy pierwszym użyciu; rejestr ma stały limit wpisów
//! (najstarsze wypadają), więc UI nie może ani podać własnej ścieżki, ani zgadnąć cudzego uchwytu.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Najwięcej żywych uchwytów naraz.
const MAX_HANDLES: usize = 16;

/// Rejestr uchwytów.
#[derive(Debug, Default)]
pub struct PathHandles {
    entries: Mutex<VecDeque<(String, PathBuf, Instant)>>,
}

impl PathHandles {
    /// Wydaje uchwyt ścieżki ważny `ttl`.
    pub fn issue(&self, path: PathBuf, ttl: Duration) -> String {
        let handle = format!("h-{}", uuid::Uuid::new_v4().simple());
        let deadline = Instant::now() + ttl;
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.retain(|(_, _, until)| *until > Instant::now());
        while entries.len() >= MAX_HANDLES {
            entries.pop_front();
        }
        entries.push_back((handle.clone(), path, deadline));
        handle
    }

    /// Zużywa uchwyt: ścieżka, gdy uchwyt istnieje i nie wygasł (inaczej `None`).
    pub fn take(&self, handle: &str) -> Option<PathBuf> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let at = entries.iter().position(|(h, _, _)| h == handle)?;
        let (_, path, until) = entries.remove(at)?;
        (until > Instant::now()).then_some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_single_use_and_expire() {
        let h = PathHandles::default();
        let a = h.issue(PathBuf::from("a.alfa"), Duration::from_secs(60));
        let b = h.issue(PathBuf::from("b.alfa"), Duration::from_secs(60));
        assert_ne!(a, b);
        assert_eq!(h.take(&a), Some(PathBuf::from("a.alfa")));
        assert_eq!(h.take(&a), None, "jednorazowy");
        assert_eq!(
            h.take("C:\\Users\\Ty\\b.alfa"),
            None,
            "ścieżka to nie uchwyt"
        );
        let gone = h.issue(PathBuf::from("c.alfa"), Duration::ZERO);
        assert_eq!(h.take(&gone), None, "wygasły");
        assert_eq!(h.take(&b), Some(PathBuf::from("b.alfa")));
    }

    #[test]
    fn registry_is_bounded() {
        let h = PathHandles::default();
        let first = h.issue(PathBuf::from("0.alfa"), Duration::from_secs(60));
        for i in 1..=MAX_HANDLES {
            h.issue(PathBuf::from(format!("{i}.alfa")), Duration::from_secs(60));
        }
        assert_eq!(h.take(&first), None, "najstarszy wypadł");
    }
}
