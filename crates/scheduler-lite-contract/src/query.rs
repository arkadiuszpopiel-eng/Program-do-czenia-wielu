//! Zapytania o stan tablicy blokad (UI: kto mówi, kto czeka; timer: najbliższy termin).

use crate::table::LockTable;
use crate::types::{LeaseId, LeaseInfo, QueuedRequest, Resource};

impl LockTable {
    /// Bieżąca dzierżawa zasobu.
    pub fn holder_of(&self, resource: &Resource) -> Option<LeaseInfo> {
        let id = self.slots.get(resource)?.lease?;
        self.leases.get(&id).map(|h| h.info.clone())
    }

    /// Dzierżawa po identyfikatorze.
    pub fn lease(&self, id: LeaseId) -> Option<LeaseInfo> {
        self.leases.get(&id).map(|h| h.info.clone())
    }

    /// Wszystkie aktywne dzierżawy.
    pub fn leases(&self) -> Vec<LeaseInfo> {
        self.leases.values().map(|h| h.info.clone()).collect()
    }

    /// Kolejka zasobu (w kolejności przyznawania).
    pub fn queue(&self, resource: &Resource) -> Vec<QueuedRequest> {
        self.slots.get(resource).map_or_else(Vec::new, |slot| {
            slot.queue
                .iter()
                .filter_map(|id| self.pending.get(id).map(|p| p.view.clone()))
                .collect()
        })
    }

    /// Liczba czekających żądań.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Brak dzierżaw, żądań i rezerwacji.
    pub fn is_idle(&self) -> bool {
        self.leases.is_empty()
            && self.pending.is_empty()
            && self.slots.values().all(|s| s.reservation.is_none())
    }

    /// Najbliższy termin (timeout żądania albo koniec rezerwacji) — dla timera sterownika.
    pub fn next_deadline(&self) -> Option<u64> {
        let pending = self.pending.values().map(|p| p.view.deadline_ms);
        let reservations = self
            .slots
            .values()
            .filter_map(|s| s.reservation.as_ref().map(|r| r.until_ms));
        pending.chain(reservations).min()
    }
}
