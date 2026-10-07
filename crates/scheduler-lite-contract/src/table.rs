//! Deterministyczny rdzeń decyzyjny (bez zegara, bez async): ta sama sekwencja wywołań z tymi
//! samymi czasami daje tę samą sekwencję efektów. `-impl` i `-fake` różnią się tylko zegarem
//! i tym, dokąd trafiają zdarzenia.

use std::collections::BTreeMap;

use crate::types::{
    Holder, LeaseId, LeaseInfo, LeaseRequest, MAX_WAIT_LIMIT, OnTimeout, PreemptReason, Priority,
    QueuedRequest, RequestId, Resource, ResourcePolicy, SchedError, millis,
};

/// Efekt decyzji — wejście dla sterownika (dostarczenie wyników, sygnały, zdarzenia).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Przyznano dzierżawę.
    Granted(LeaseInfo),
    /// Żądanie czeka w kolejce.
    Queued {
        /// Żądanie.
        request: QueuedRequest,
        /// Zasób.
        resource: Resource,
        /// Pozycja w kolejce (0 = pierwsze).
        position: usize,
    },
    /// Posiadaczka proszona o zwolnienie w punkcie atomowym.
    PreemptRequested {
        /// Dzierżawa.
        lease: LeaseInfo,
        /// Kto czeka.
        by: Holder,
        /// Powód.
        reason: PreemptReason,
    },
    /// Dzierżawa zwolniona przez posiadaczkę.
    Released(LeaseInfo),
    /// Dzierżawa przekazana innej posiadaczce (stara wygasa bez luki).
    HandedOff {
        /// Stara dzierżawa.
        lease: LeaseInfo,
        /// Adresatka.
        to: Holder,
    },
    /// Dzierżawa odebrana (kill-switch).
    Revoked(LeaseInfo, PreemptReason),
    /// Żądanie przekroczyło `max_wait`.
    TimedOut {
        /// Żądanie.
        request: QueuedRequest,
        /// Zasób.
        resource: Resource,
        /// Zachowanie.
        on_timeout: OnTimeout,
    },
    /// Żądanie odrzucone jako najmłodsze w cyklu oczekiwania.
    Deadlock {
        /// Żądanie.
        request: QueuedRequest,
        /// Zasób.
        resource: Resource,
        /// Posiadaczki w cyklu.
        cycle: Vec<Holder>,
    },
    /// Żądanie anulowane (kill-switch albo porzucone przez wołającą).
    Cancelled {
        /// Żądanie.
        request: QueuedRequest,
        /// Zasób.
        resource: Resource,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub(crate) resource: Resource,
    pub(crate) view: QueuedRequest,
    pub(crate) on_timeout: OnTimeout,
}

#[derive(Debug, Clone)]
pub(crate) struct Held {
    pub(crate) info: LeaseInfo,
    pub(crate) preempt_sent: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Reservation {
    pub(crate) holder: Holder,
    pub(crate) until_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Slot {
    pub(crate) lease: Option<LeaseId>,
    pub(crate) queue: Vec<RequestId>,
    pub(crate) reservation: Option<Reservation>,
}

/// Tablica blokad: posiadaczki, kolejki priorytetowe (priorytet malejąco, potem FIFO),
/// rezerwacje przekazania i wykrywanie zakleszczeń.
#[derive(Debug, Clone, Default)]
pub struct LockTable {
    pub(crate) policies: BTreeMap<Resource, ResourcePolicy>,
    pub(crate) slots: BTreeMap<Resource, Slot>,
    pub(crate) pending: BTreeMap<RequestId, Pending>,
    pub(crate) leases: BTreeMap<LeaseId, Held>,
    next_request: u64,
    next_lease: u64,
}

impl LockTable {
    /// Pusta tablica z politykami domyślnymi.
    pub fn new() -> Self {
        Self::default()
    }

    /// Nadpisuje politykę zasobu.
    pub fn set_policy(&mut self, resource: Resource, policy: ResourcePolicy) {
        self.policies.insert(resource, policy);
    }

    /// Polityka zasobu.
    pub fn policy(&self, resource: &Resource) -> ResourcePolicy {
        self.policies
            .get(resource)
            .copied()
            .unwrap_or_else(|| ResourcePolicy::default_for(resource))
    }

    /// Zgłasza żądanie: przyznanie od razu, kolejka albo natychmiastowy błąd.
    pub fn request(
        &mut self,
        req: LeaseRequest,
        now_ms: u64,
    ) -> Result<(RequestId, Vec<Effect>), SchedError> {
        if req.max_wait > MAX_WAIT_LIMIT {
            return Err(SchedError::InvalidMaxWait {
                max_ms: millis(req.max_wait),
            });
        }
        if req.resource == Resource::Speaker && matches!(req.holder, Holder::System(_)) {
            return Err(SchedError::SystemCannotSpeak);
        }
        if self
            .holder_of(&req.resource)
            .is_some_and(|l| l.holder == req.holder)
        {
            return Err(SchedError::AlreadyHeld {
                resource: req.resource,
                holder: req.holder,
            });
        }
        let id = self.next_request_id();
        let on_timeout = req
            .on_timeout
            .unwrap_or_else(|| self.policy(&req.resource).on_timeout);
        let view = QueuedRequest {
            id,
            holder: req.holder,
            priority: req.priority,
            enqueued_at_ms: now_ms,
            deadline_ms: now_ms.saturating_add(millis(req.max_wait)),
        };
        let slot = self.slots.entry(req.resource.clone()).or_default();
        let active = slot.reservation.as_ref().filter(|r| r.until_ms > now_ms);
        let reserved_for_me = active.is_some_and(|r| r.holder == view.holder);
        let reserved_for_other = active.is_some_and(|r| r.holder != view.holder);
        if slot.lease.is_none() && !reserved_for_other && (slot.queue.is_empty() || reserved_for_me)
        {
            slot.reservation = None;
            let info = self.grant(&req.resource, view, now_ms);
            return Ok((id, vec![Effect::Granted(info)]));
        }
        if req.max_wait.is_zero() {
            return Ok((
                id,
                vec![Effect::TimedOut {
                    request: view,
                    resource: req.resource,
                    on_timeout,
                }],
            ));
        }
        self.pending.insert(
            id,
            Pending {
                resource: req.resource.clone(),
                view: view.clone(),
                on_timeout,
            },
        );
        let position = self.enqueue(&req.resource, id);
        let mut effects = vec![Effect::Queued {
            request: view,
            resource: req.resource.clone(),
            position,
        }];
        // Rezerwacja przekazania dla tej żądającej: przyznaj bez luki.
        effects.extend(self.grant_next(&req.resource, now_ms));
        effects.extend(self.check_preempt(&req.resource));
        effects.extend(self.resolve_deadlocks());
        Ok((id, effects))
    }

    /// Zwolnienie dzierżawy (drop / `release`); nieznana dzierżawa → brak efektów.
    pub fn release(&mut self, lease: LeaseId, now_ms: u64) -> Vec<Effect> {
        let Some(held) = self.take_lease(lease) else {
            return Vec::new();
        };
        let resource = held.info.resource.clone();
        let mut effects = vec![Effect::Released(held.info)];
        effects.extend(self.after_free(&resource, now_ms));
        effects
    }

    /// Porzucenie czekającego żądania przez wołającą.
    pub fn cancel(&mut self, request: RequestId, now_ms: u64) -> Vec<Effect> {
        let Some(p) = self.remove_pending(request) else {
            return Vec::new();
        };
        let resource = p.resource.clone();
        let mut effects = vec![Effect::Cancelled {
            request: p.view,
            resource: p.resource,
        }];
        effects.extend(self.grant_next(&resource, now_ms));
        effects
    }

    /// Kolejny identyfikator żądania (monotoniczny).
    pub(crate) fn next_request_id(&mut self) -> RequestId {
        self.next_request += 1;
        RequestId(self.next_request)
    }

    pub(crate) fn grant(
        &mut self,
        resource: &Resource,
        view: QueuedRequest,
        now_ms: u64,
    ) -> LeaseInfo {
        self.next_lease += 1;
        let info = LeaseInfo {
            id: LeaseId(self.next_lease),
            request: view.id,
            resource: resource.clone(),
            holder: view.holder,
            priority: view.priority,
            granted_at_ms: now_ms,
        };
        self.slots.entry(resource.clone()).or_default().lease = Some(info.id);
        self.leases.insert(
            info.id,
            Held {
                info: info.clone(),
                preempt_sent: false,
            },
        );
        info
    }

    pub(crate) fn take_lease(&mut self, lease: LeaseId) -> Option<Held> {
        let held = self.leases.remove(&lease)?;
        if let Some(slot) = self.slots.get_mut(&held.info.resource)
            && slot.lease == Some(lease)
        {
            slot.lease = None;
        }
        Some(held)
    }

    pub(crate) fn remove_pending(&mut self, request: RequestId) -> Option<Pending> {
        let p = self.pending.remove(&request)?;
        if let Some(slot) = self.slots.get_mut(&p.resource) {
            slot.queue.retain(|id| *id != request);
        }
        Some(p)
    }

    /// Wstawia do kolejki: priorytet malejąco, w obrębie priorytetu FIFO. Zwraca pozycję.
    fn enqueue(&mut self, resource: &Resource, id: RequestId) -> usize {
        let priority_of = |pending: &BTreeMap<RequestId, Pending>, r: &RequestId| {
            pending
                .get(r)
                .map_or(Priority::Background, |p| p.view.priority)
        };
        let mine = priority_of(&self.pending, &id);
        let pending = &self.pending;
        let slot = self.slots.entry(resource.clone()).or_default();
        let position = slot
            .queue
            .iter()
            .position(|r| priority_of(pending, r) < mine)
            .unwrap_or(slot.queue.len());
        slot.queue.insert(position, id);
        position
    }

    /// Po zwolnieniu zasobu: przyznaj następnej, sprawdź wywłaszczenie i zakleszczenia.
    pub(crate) fn after_free(&mut self, resource: &Resource, now_ms: u64) -> Vec<Effect> {
        let mut effects = self.grant_next(resource, now_ms);
        effects.extend(self.check_preempt(resource));
        effects.extend(self.resolve_deadlocks());
        effects
    }

    /// Przyznaje wolny zasób: najpierw adresatce rezerwacji, inaczej głowie kolejki.
    pub(crate) fn grant_next(&mut self, resource: &Resource, now_ms: u64) -> Vec<Effect> {
        let Some(slot) = self.slots.get_mut(resource) else {
            return Vec::new();
        };
        if slot.lease.is_some() {
            return Vec::new();
        }
        if slot
            .reservation
            .as_ref()
            .is_some_and(|r| r.until_ms <= now_ms)
        {
            slot.reservation = None;
        }
        let next = match &slot.reservation {
            Some(r) => slot
                .queue
                .iter()
                .find(|id| {
                    self.pending
                        .get(id)
                        .is_some_and(|p| p.view.holder == r.holder)
                })
                .copied(),
            None => slot.queue.first().copied(),
        };
        let Some(id) = next else {
            return Vec::new();
        };
        slot.reservation = None;
        let Some(p) = self.remove_pending(id) else {
            return Vec::new();
        };
        vec![Effect::Granted(self.grant(resource, p.view, now_ms))]
    }

    /// Wywłaszczenie w punkcie atomowym: czeka ktoś ważniejszy niż posiadaczka.
    pub(crate) fn check_preempt(&mut self, resource: &Resource) -> Vec<Effect> {
        if !self.policy(resource).preemptible_at_atomic {
            return Vec::new();
        }
        let Some(slot) = self.slots.get(resource) else {
            return Vec::new();
        };
        let (Some(lease), Some(head)) = (slot.lease, slot.queue.first()) else {
            return Vec::new();
        };
        let Some(waiting) = self.pending.get(head).map(|p| p.view.clone()) else {
            return Vec::new();
        };
        let Some(held) = self.leases.get_mut(&lease) else {
            return Vec::new();
        };
        if held.preempt_sent || waiting.priority <= held.info.priority {
            return Vec::new();
        }
        held.preempt_sent = true;
        let reason = if waiting.priority == Priority::UserSpeech {
            PreemptReason::UserSpeaks
        } else {
            PreemptReason::HigherPriority
        };
        vec![Effect::PreemptRequested {
            lease: held.info.clone(),
            by: waiting.holder,
            reason,
        }]
    }
}
