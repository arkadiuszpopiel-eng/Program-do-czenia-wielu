//! Operacje tablicy blokad: wywłaszczenie, przekazanie (handoff), upływ czasu, kill-switch.

use crate::table::{Effect, LockTable, Reservation};
use crate::types::{Holder, LeaseId, PreemptReason, Resource, SchedError};

impl LockTable {
    /// Wywłaszczenie posiadaczki zasobu. `KillSwitch` odbiera dzierżawę od razu; pozostałe powody
    /// tylko sygnalizują `PreemptRequested` (zasób wywłaszczalny w punkcie atomowym).
    pub fn preempt(
        &mut self,
        resource: &Resource,
        by: Holder,
        reason: PreemptReason,
        now_ms: u64,
    ) -> Result<Vec<Effect>, SchedError> {
        let lease = self
            .slots
            .get(resource)
            .and_then(|s| s.lease)
            .ok_or_else(|| SchedError::NotHeld(resource.clone()))?;
        if reason == PreemptReason::KillSwitch {
            let Some(held) = self.take_lease(lease) else {
                return Ok(Vec::new());
            };
            let mut effects = vec![Effect::Revoked(held.info, reason)];
            effects.extend(self.after_free(resource, now_ms));
            return Ok(effects);
        }
        if !self.policy(resource).preemptible_at_atomic {
            return Err(SchedError::NotPreemptible(resource.clone()));
        }
        let Some(held) = self.leases.get_mut(&lease) else {
            return Ok(Vec::new());
        };
        if held.preempt_sent {
            return Ok(Vec::new());
        }
        held.preempt_sent = true;
        Ok(vec![Effect::PreemptRequested {
            lease: held.info.clone(),
            by,
            reason,
        }])
    }

    /// Przekazanie dzierżawy bez luki: czekające żądanie adresatki dostaje zasób w tej samej
    /// chwili; jeśli adresatka jeszcze nie prosi — zasób jest dla niej zarezerwowany.
    pub fn handoff(
        &mut self,
        lease: LeaseId,
        to: Holder,
        now_ms: u64,
    ) -> Result<Vec<Effect>, SchedError> {
        let info = self.lease(lease).ok_or(SchedError::UnknownLease)?;
        if info.resource == Resource::Speaker && matches!(to, Holder::System(_)) {
            return Err(SchedError::SystemCannotSpeak);
        }
        if info.holder == to {
            return Ok(Vec::new());
        }
        let Some(held) = self.take_lease(lease) else {
            return Err(SchedError::UnknownLease);
        };
        let resource = held.info.resource.clone();
        let until_ms = now_ms.saturating_add(self.policy(&resource).handoff_reserve_ms);
        self.slots.entry(resource.clone()).or_default().reservation = Some(Reservation {
            holder: to.clone(),
            until_ms,
        });
        let mut effects = vec![Effect::HandedOff {
            lease: held.info,
            to,
        }];
        effects.extend(self.after_free(&resource, now_ms));
        Ok(effects)
    }

    /// Przekazanie zasobu trzymanego przez `from` (delegacja v0: „Przekazuję Delcie…”).
    pub fn handoff_from(
        &mut self,
        resource: &Resource,
        from: &Holder,
        to: Holder,
        now_ms: u64,
    ) -> Result<Vec<Effect>, SchedError> {
        let lease = self
            .holder_of(resource)
            .filter(|l| &l.holder == from)
            .ok_or_else(|| SchedError::NotHeld(resource.clone()))?;
        self.handoff(lease.id, to, now_ms)
    }

    /// Upływ czasu: timeouty żądań (w kolejności id) i wygasłe rezerwacje.
    pub fn tick(&mut self, now_ms: u64) -> Vec<Effect> {
        let expired: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, p)| p.view.deadline_ms <= now_ms)
            .map(|(id, _)| *id)
            .collect();
        let mut effects = Vec::new();
        let mut touched: Vec<Resource> = Vec::new();
        for id in expired {
            if let Some(p) = self.remove_pending(id) {
                touched.push(p.resource.clone());
                effects.push(Effect::TimedOut {
                    request: p.view,
                    resource: p.resource,
                    on_timeout: p.on_timeout,
                });
            }
        }
        for (resource, slot) in &mut self.slots {
            if slot
                .reservation
                .as_ref()
                .is_some_and(|r| r.until_ms <= now_ms)
            {
                slot.reservation = None;
                touched.push(resource.clone());
            }
        }
        touched.sort();
        touched.dedup();
        for resource in touched {
            effects.extend(self.after_free(&resource, now_ms));
        }
        effects
    }

    /// Kill-switch: odbiera wszystkie dzierżawy, anuluje żądania, czyści rezerwacje.
    pub fn kill_all(&mut self) -> Vec<Effect> {
        let leases: Vec<LeaseId> = self.leases.keys().copied().collect();
        let mut effects: Vec<Effect> = leases
            .into_iter()
            .filter_map(|id| self.take_lease(id))
            .map(|held| Effect::Revoked(held.info, PreemptReason::KillSwitch))
            .collect();
        let pending: Vec<_> = self.pending.keys().copied().collect();
        for id in pending {
            if let Some(p) = self.remove_pending(id) {
                effects.push(Effect::Cancelled {
                    request: p.view,
                    resource: p.resource,
                });
            }
        }
        for slot in self.slots.values_mut() {
            slot.reservation = None;
        }
        effects
    }
}
