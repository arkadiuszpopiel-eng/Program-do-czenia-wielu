//! Atrapa zarządcy rezydencji (docs/modules/model-residency/SPEC.md §Fake): te same reguły co
//! `-impl` (maszyna stanów [`LeaseTable`] z kontraktu), ręczny zegar, wstrzykiwanie błędów,
//! zapis wywołań i zdarzeń — dla testów `voice-*`, `providers-local`, `search`.
//!
//! Tylko jako `dev-dependency` innych modułów (crates/README.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use model_residency_contract::{
    Budget, Change, Grant, Lease, LeaseId, LeaseListener, LeaseRequest, LeaseTable, ManualClock,
    Mode, Residency, ResidencyClock, ResidencyError, ResidencyEvent, ResidencyState, Revocation,
};

/// Wywołanie zarejestrowane przez atrapę.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    /// `acquire` z żądaniem i wynikiem (id albo błąd).
    Acquire(LeaseRequest, Result<LeaseId, ResidencyError>),
    /// `release`.
    Release(LeaseId),
    /// `set_mode`.
    SetMode(Mode),
}

struct State {
    table: LeaseTable,
    listeners: BTreeMap<String, Arc<dyn LeaseListener>>,
    fail_next: VecDeque<ResidencyError>,
    calls: Vec<Call>,
    events: Vec<ResidencyEvent>,
}

/// Deterministyczny zarządca rezydencji.
#[derive(Clone)]
pub struct FakeResidency {
    state: Arc<Mutex<State>>,
    clock: Arc<ManualClock>,
}

impl FakeResidency {
    /// Atrapa z budżetem i zegarem od 0 ms.
    pub fn new(budget: Budget) -> Self {
        let state = State {
            table: LeaseTable::new(budget),
            listeners: BTreeMap::new(),
            fail_next: VecDeque::new(),
            calls: Vec::new(),
            events: Vec::new(),
        };
        Self {
            state: Arc::new(Mutex::new(state)),
            clock: Arc::new(ManualClock::new()),
        }
    }

    /// Budżet baseline (8 GB VRAM − 768 MB pulpitu, 8 GB RAM dla modeli).
    pub fn baseline() -> Self {
        Self::new(Budget {
            vram_mb: 8_192 - 768,
            ram_mb: 8_192,
            desktop_reserve_mb: 768,
            stt_tts_exclusive: false,
        })
    }

    /// Zegar atrapy (przesuwany ręcznie).
    pub fn clock(&self) -> Arc<ManualClock> {
        Arc::clone(&self.clock)
    }

    /// Następne `acquire` zwróci ten błąd (kolejka).
    pub fn fail_next(&self, error: ResidencyError) {
        self.lock().fail_next.push_back(error);
    }

    /// Zarejestrowane wywołania.
    pub fn calls(&self) -> Vec<Call> {
        self.lock().calls.clone()
    }

    /// Zdarzenia `residency.*` w kolejności.
    pub fn events(&self) -> Vec<ResidencyEvent> {
        self.lock().events.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn with_table<T>(&self, f: impl FnOnce(&mut LeaseTable, u64) -> T) -> T {
        let now = self.clock.now_ms();
        f(&mut self.lock().table, now)
    }

    fn notify(&self, changes: &[Change]) {
        let listeners = self.lock().listeners.clone();
        for change in changes {
            match change {
                Change::Revoked(r) => {
                    if let Some(l) = listeners.get(&r.lease.request.owner) {
                        l.revoked(r);
                    }
                }
                Change::Moved(lease) => {
                    if let Some(l) = listeners.get(&lease.request.owner) {
                        l.moved(lease);
                    }
                }
            }
        }
        self.lock()
            .events
            .extend(ResidencyEvent::from_changes(changes));
    }
}

impl Residency for FakeResidency {
    fn acquire(&self, request: LeaseRequest) -> Result<Grant, ResidencyError> {
        let injected = self.lock().fail_next.pop_front();
        let result = match injected {
            Some(e) => Err(e),
            None => self.with_table(|t, now| t.acquire(&request, now)),
        };
        let recorded = result.as_ref().map(|g| g.lease.id).map_err(Clone::clone);
        self.lock()
            .calls
            .push(Call::Acquire(request.clone(), recorded));
        if let Ok(grant) = &result {
            let changes: Vec<Change> = grant.evicted.iter().cloned().map(Change::Revoked).collect();
            self.notify(&changes);
            let mut events = ResidencyEvent::from_grant(grant, true);
            events.retain(|e| !matches!(e, ResidencyEvent::Evicted { .. }));
            self.lock().events.extend(events);
        }
        result
    }

    fn release(&self, id: LeaseId) -> Result<(), ResidencyError> {
        let lease = self.with_table(|t, _| t.release(id))?;
        let mut st = self.lock();
        st.calls.push(Call::Release(id));
        st.events.push(ResidencyEvent::Released { lease });
        Ok(())
    }

    fn touch(&self, id: LeaseId) -> Result<(), ResidencyError> {
        self.with_table(|t, now| t.touch(id, now))
    }

    fn set_in_use(&self, id: LeaseId, in_use: bool) -> Result<(), ResidencyError> {
        self.with_table(|t, now| t.set_in_use(id, in_use, now))
    }

    fn lease(&self, id: LeaseId) -> Option<Lease> {
        self.with_table(|t, _| t.lease(id).cloned())
    }

    fn snapshot(&self) -> ResidencyState {
        self.with_table(|t, _| t.snapshot())
    }

    fn set_mode(&self, mode: Mode) -> Vec<Change> {
        let (from, changes) = self.with_table(|t, _| (t.mode(), t.set_mode(mode)));
        {
            let mut st = self.lock();
            st.calls.push(Call::SetMode(mode));
            st.events
                .push(ResidencyEvent::ModeChanged { from, to: mode });
        }
        self.notify(&changes);
        changes
    }

    fn set_budget(&self, budget: Budget) -> Vec<Change> {
        let changes = self.with_table(|t, _| t.set_budget(budget));
        self.notify(&changes);
        changes
    }

    fn reap_idle(&self) -> Vec<Revocation> {
        let reaped = self.with_table(|t, now| t.reap_idle(now));
        let changes: Vec<Change> = reaped.iter().cloned().map(Change::Revoked).collect();
        self.notify(&changes);
        reaped
    }

    fn listen(&self, owner: &str, listener: Arc<dyn LeaseListener>) {
        self.lock().listeners.insert(owner.to_owned(), listener);
    }
}
