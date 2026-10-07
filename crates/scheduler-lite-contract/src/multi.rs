//! Atomowe przyznanie kompletu zasobów („wszystko albo nic”, bez czekania) — rozszerzenie dla
//! pełnego `scheduler` (F5). Zadanie dostaje wszystkie zasoby naraz albo żadnego, więc nigdy nie
//! czeka, trzymając część z nich: brak warunku „hold and wait” = brak zakleszczeń między zadaniami.
//! Żądania czekające w kolejce (`acquire`, np. mowa) mają pierwszeństwo — zasób z niepustą
//! kolejką albo z cudzą rezerwacją przekazania nie jest wolny.

use std::collections::BTreeSet;

use crate::table::{Effect, LockTable};
use crate::types::{
    Holder, LeaseInfo, LeaseRequest, MAX_WAIT_LIMIT, QueuedRequest, Resource, SchedError, millis,
};

impl LockTable {
    /// Czy `resource` jest teraz wolny dla `holder`: brak posiadaczki, pusta kolejka i brak
    /// rezerwacji przekazania dla kogoś innego.
    pub fn is_free_for(&self, resource: &Resource, holder: &Holder, now_ms: u64) -> bool {
        let Some(slot) = self.slots.get(resource) else {
            return true;
        };
        let reserved_for_other = slot
            .reservation
            .as_ref()
            .is_some_and(|r| r.until_ms > now_ms && &r.holder != holder);
        slot.lease.is_none() && slot.queue.is_empty() && !reserved_for_other
    }

    /// Przyznaje wszystkie żądania naraz albo żadnego. Błędy: zasób zajęty → `Timeout`
    /// z `waited_ms: 0` (pierwszy zajęty), powtórzony zasób → `AlreadyHeld`, usługa systemowa
    /// i głośnik → `SystemCannotSpeak`, `max_wait` ponad limit → `InvalidMaxWait`.
    pub fn grant_all(
        &mut self,
        requests: &[LeaseRequest],
        now_ms: u64,
    ) -> Result<Vec<Effect>, SchedError> {
        let mut seen = BTreeSet::new();
        for req in requests {
            if req.max_wait > MAX_WAIT_LIMIT {
                return Err(SchedError::InvalidMaxWait {
                    max_ms: millis(req.max_wait),
                });
            }
            if req.resource == Resource::Speaker && matches!(req.holder, Holder::System(_)) {
                return Err(SchedError::SystemCannotSpeak);
            }
            if !seen.insert(req.resource.clone()) {
                return Err(SchedError::AlreadyHeld {
                    resource: req.resource.clone(),
                    holder: req.holder.clone(),
                });
            }
        }
        if let Some(busy) = requests
            .iter()
            .find(|r| !self.is_free_for(&r.resource, &r.holder, now_ms))
        {
            return Err(SchedError::Timeout {
                resource: busy.resource.clone(),
                on_timeout: busy
                    .on_timeout
                    .unwrap_or_else(|| self.policy(&busy.resource).on_timeout),
                waited_ms: 0,
            });
        }
        let mut effects = Vec::with_capacity(requests.len());
        for req in requests {
            let id = self.next_request_id();
            let view = QueuedRequest {
                id,
                holder: req.holder.clone(),
                priority: req.priority,
                enqueued_at_ms: now_ms,
                deadline_ms: now_ms,
            };
            if let Some(slot) = self.slots.get_mut(&req.resource) {
                slot.reservation = None;
            }
            let info: LeaseInfo = self.grant(&req.resource, view, now_ms);
            effects.push(Effect::Granted(info));
        }
        Ok(effects)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use personas_contract::PersonaId;

    use super::*;
    use crate::types::{OnTimeout, Priority};

    fn req(resource: Resource, who: &str) -> LeaseRequest {
        LeaseRequest::new(
            resource,
            Holder::Persona(PersonaId::new(who)),
            Priority::Normal,
            Duration::ZERO,
        )
    }

    #[test]
    fn all_or_nothing() {
        let mut t = LockTable::new();
        let file = Resource::file("C:\\Dane\\a.txt");
        let granted = t
            .grant_all(
                &[req(Resource::Speaker, "alfa"), req(file.clone(), "alfa")],
                5,
            )
            .unwrap();
        assert_eq!(granted.len(), 2);
        assert!(!t.is_free_for(&Resource::Speaker, &Holder::User, 5));
        // Drugi komplet nachodzi na plik → nic nie zostaje przyznane (głośnik ekranu wolny).
        let err = t
            .grant_all(
                &[
                    req(Resource::ScreenInput, "beta"),
                    req(file.clone(), "beta"),
                ],
                6,
            )
            .unwrap_err();
        assert_eq!(
            err,
            SchedError::Timeout {
                resource: file,
                on_timeout: OnTimeout::Fail,
                waited_ms: 0
            }
        );
        assert!(t.is_free_for(&Resource::ScreenInput, &Holder::User, 6));
        assert_eq!(t.leases().len(), 2);
    }

    #[test]
    fn validation() {
        let mut t = LockTable::new();
        let sys = LeaseRequest::new(
            Resource::Speaker,
            Holder::System("x".into()),
            Priority::Normal,
            Duration::ZERO,
        );
        assert_eq!(t.grant_all(&[sys], 0), Err(SchedError::SystemCannotSpeak));
        let dup = [req(Resource::Mic, "a"), req(Resource::Mic, "a")];
        assert!(matches!(
            t.grant_all(&dup, 0),
            Err(SchedError::AlreadyHeld { .. })
        ));
        let mut long = req(Resource::Mic, "a");
        long.max_wait = MAX_WAIT_LIMIT + Duration::from_secs(1);
        assert!(matches!(
            t.grant_all(&[long], 0),
            Err(SchedError::InvalidMaxWait { .. })
        ));
        assert!(t.grant_all(&[], 0).unwrap().is_empty());
        assert!(t.is_idle());
    }

    #[test]
    fn queue_and_reservation_block() {
        let mut t = LockTable::new();
        let (_, e) = t.request(req(Resource::Speaker, "alfa"), 0).unwrap();
        let Effect::Granted(lease) = &e[0] else {
            panic!("brak przyznania");
        };
        let mut waiting = req(Resource::Speaker, "beta");
        waiting.max_wait = Duration::from_secs(5);
        t.request(waiting, 0).unwrap();
        // Zwolnienie przyznaje kolejce, nie kompletowi.
        t.release(lease.id, 1);
        assert!(t.grant_all(&[req(Resource::Speaker, "gama")], 1).is_err());
        // Rezerwacja przekazania blokuje innych, ale nie adresatkę.
        let holder = t.holder_of(&Resource::Speaker).unwrap();
        t.handoff(holder.id, Holder::Persona(PersonaId::new("delta")), 2)
            .unwrap();
        assert!(!t.is_free_for(&Resource::Speaker, &Holder::User, 3));
        let delta = Holder::Persona(PersonaId::new("delta"));
        assert!(t.is_free_for(&Resource::Speaker, &delta, 3));
        assert_eq!(
            t.grant_all(&[req(Resource::Speaker, "delta")], 3)
                .unwrap()
                .len(),
            1
        );
    }
}
