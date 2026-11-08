//! Mała pamięć podręczna wyników `recall` (agentka pyta wiele razy w jednej turze).
//! Każdy zapis w pamięci czyści ją w całości — `forget` raportuje liczbę wyczyszczonych pozycji.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Utc};

use crate::types::Recalled;

/// Pojemność (najstarsze pozycje wypadają).
pub const CACHE_CAPACITY: usize = 64;
/// Ważność pozycji w sekundach (TTL wpisów liczony jest względem czasu zapytania).
pub const CACHE_TTL_SECS: i64 = 30;

#[derive(Debug)]
struct Slot {
    key: String,
    at: DateTime<Utc>,
    hits: Vec<Recalled>,
}

/// Pamięć podręczna `klucz → wyniki`.
#[derive(Debug, Default)]
pub(crate) struct RecallCache {
    slots: Mutex<VecDeque<Slot>>,
}

impl RecallCache {
    /// Wynik z pamięci podręcznej, jeśli świeży.
    pub(crate) fn get(&self, key: &str, now: DateTime<Utc>) -> Option<Vec<Recalled>> {
        let slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots
            .iter()
            .find(|s| s.key == key && (now - s.at).num_seconds().abs() <= CACHE_TTL_SECS)
            .map(|s| s.hits.clone())
    }

    /// Zapamiętuje wynik.
    pub(crate) fn put(&self, key: String, now: DateTime<Utc>, hits: Vec<Recalled>) {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots.retain(|s| s.key != key);
        if slots.len() >= CACHE_CAPACITY {
            slots.pop_front();
        }
        slots.push_back(Slot { key, at: now, hits });
    }

    /// Czyści wszystko; zwraca liczbę usuniętych pozycji.
    pub(crate) fn clear(&self) -> usize {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let n = slots.len();
        slots.clear();
        n
    }

    /// Liczba pozycji.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_ttl_and_clear() {
        let c = RecallCache::default();
        let t = DateTime::<Utc>::default();
        for i in 0..(CACHE_CAPACITY + 5) {
            c.put(format!("k{i}"), t, Vec::new());
        }
        assert_eq!(c.len(), CACHE_CAPACITY);
        assert!(c.get("k0", t).is_none());
        assert!(c.get("k70", t).is_none() && c.get("k68", t).is_some());
        let later = t + chrono::Duration::seconds(CACHE_TTL_SECS + 1);
        assert!(c.get("k68", later).is_none());
        assert_eq!(c.clear(), CACHE_CAPACITY);
        assert_eq!(c.len(), 0);
    }
}
