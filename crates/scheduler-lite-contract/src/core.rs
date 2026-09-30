//! Sterownik asynchroniczny wokół [`LockTable`]: oczekujące żądania (oneshot), sygnały dzierżaw
//! (watch), zdarzenia. Parametryzowany [`Host`]em — zegar i ujście zdarzeń: `-impl` (tokio +
//! magistrala) i `-fake` (wirtualny zegar + nagranie). Wyniki są dostarczane **po** zwolnieniu
//! blokady, więc drop niedostarczonej dzierżawy (zwolnienie) nie powoduje zakleszczenia.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use core_bus_contract::Event;
use tokio::sync::{oneshot, watch};

use crate::events::effect_event;
use crate::lease::{Lease, LeaseControl};
use crate::table::{Effect, LockTable};
use crate::types::{
    Holder, LeaseId, LeaseInfo, LeaseRequest, LeaseSignal, PreemptReason, QueuedRequest, RequestId,
    Resource, ResourcePolicy, SchedError,
};

/// Otoczenie sterownika: zegar (ms, monotoniczny) i ujście zdarzeń.
pub trait Host: Send + Sync + 'static {
    /// Bieżący czas (ms).
    fn now_ms(&self) -> u64;
    /// Zdarzenia do opublikowania (w kolejności decyzji).
    fn emit(&self, events: Vec<Event>);
    /// Zmienił się najbliższy termin (timer sterownika powinien się przestawić).
    fn deadline_changed(&self) {}
}

type Waiter = oneshot::Sender<Result<Lease, SchedError>>;

#[derive(Default)]
struct Engine {
    table: LockTable,
    waiters: BTreeMap<RequestId, Waiter>,
    signals: BTreeMap<LeaseId, watch::Sender<LeaseSignal>>,
}

/// Rzeczy do zrobienia po zwolnieniu blokady.
#[derive(Default)]
struct Outcome {
    deliveries: Vec<(Waiter, Result<Lease, SchedError>)>,
    orphans: Vec<Lease>,
    events: Vec<Event>,
}

/// Rdzeń schedulera współdzielony przez `-impl` i `-fake`.
pub struct Core<H: Host> {
    engine: Mutex<Engine>,
    host: H,
    me: Weak<Core<H>>,
}

/// Anuluje żądanie, gdy wołająca porzuci `acquire` przed wynikiem.
struct CancelOnDrop<'a, H: Host> {
    core: &'a Core<H>,
    request: RequestId,
    armed: bool,
}

impl<H: Host> Drop for CancelOnDrop<'_, H> {
    fn drop(&mut self) {
        if self.armed {
            LeaseControl::cancel(self.core, self.request);
        }
    }
}

impl<H: Host> Core<H> {
    /// Nowy rdzeń.
    pub fn new(host: H) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            engine: Mutex::new(Engine::default()),
            host,
            me: me.clone(),
        })
    }

    /// Otoczenie (zegar, ujście zdarzeń).
    pub fn host(&self) -> &H {
        &self.host
    }

    fn lock(&self) -> MutexGuard<'_, Engine> {
        self.engine.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn control(&self) -> Weak<dyn LeaseControl> {
        self.me.clone()
    }

    /// Żąda zasobu; czeka najwyżej `max_wait`. Porzucenie przyszłości anuluje żądanie.
    pub async fn acquire(&self, request: LeaseRequest) -> Result<Lease, SchedError> {
        let now = self.host.now_ms();
        let mut out = Outcome::default();
        let waiting = {
            let mut engine = self.lock();
            let (id, effects) = engine.table.request(request, now)?;
            match self.apply(&mut engine, effects, Some(id), now, &mut out) {
                Some(ready) => Err(ready),
                None => {
                    let (tx, rx) = oneshot::channel();
                    engine.waiters.insert(id, tx);
                    Ok((id, rx))
                }
            }
        };
        self.finish(out);
        let (id, rx) = match waiting {
            Ok(wait) => wait,
            Err(ready) => return ready,
        };
        let mut guard = CancelOnDrop {
            core: self,
            request: id,
            armed: true,
        };
        let result = rx.await.unwrap_or(Err(SchedError::Cancelled));
        guard.armed = false;
        result
    }

    /// Wywłaszczenie (np. `UserSpeaks` wobec narracji). `KillSwitch` odbiera dzierżawę.
    pub fn preempt(
        &self,
        resource: &Resource,
        by: Holder,
        reason: PreemptReason,
    ) -> Result<(), SchedError> {
        self.run(|table, now| table.preempt(resource, by, reason, now))
    }

    /// Przekazanie zasobu trzymanego przez `from` (delegacja v0).
    pub fn handoff_from(
        &self,
        resource: &Resource,
        from: &Holder,
        to: Holder,
    ) -> Result<(), SchedError> {
        self.run(|table, now| table.handoff_from(resource, from, to, now))
    }

    /// Upływ czasu: timeouty i wygasłe rezerwacje.
    pub fn tick(&self) {
        let _ = self.run(|table, now| Ok(table.tick(now)));
    }

    /// Kill-switch: odbiera wszystko, czyści kolejki. Zwraca liczbę odebranych i anulowanych.
    pub fn kill_all(&self) -> usize {
        let mut count = 0;
        let _ = self.run(|table, _| {
            let effects = table.kill_all();
            count = effects.len();
            Ok(effects)
        });
        count
    }

    /// Nadpisuje politykę zasobu.
    pub fn set_policy(&self, resource: Resource, policy: ResourcePolicy) {
        self.lock().table.set_policy(resource, policy);
    }

    /// Bieżąca posiadaczka zasobu.
    pub fn holder(&self, resource: &Resource) -> Option<LeaseInfo> {
        self.lock().table.holder_of(resource)
    }

    /// Kolejka zasobu.
    pub fn queue(&self, resource: &Resource) -> Vec<QueuedRequest> {
        self.lock().table.queue(resource)
    }

    /// Wszystkie aktywne dzierżawy.
    pub fn leases(&self) -> Vec<LeaseInfo> {
        self.lock().table.leases()
    }

    /// Najbliższy termin (ms).
    pub fn next_deadline(&self) -> Option<u64> {
        self.lock().table.next_deadline()
    }

    /// Kopia tablicy (diagnostyka i testy własności).
    pub fn snapshot(&self) -> LockTable {
        self.lock().table.clone()
    }

    fn run(
        &self,
        f: impl FnOnce(&mut LockTable, u64) -> Result<Vec<Effect>, SchedError>,
    ) -> Result<(), SchedError> {
        let now = self.host.now_ms();
        let mut out = Outcome::default();
        {
            let mut engine = self.lock();
            let effects = f(&mut engine.table, now)?;
            self.apply(&mut engine, effects, None, now, &mut out);
        }
        self.finish(out);
        Ok(())
    }

    fn finish(&self, out: Outcome) {
        for (waiter, result) in out.deliveries {
            // Wołająca już nie czeka → niedostarczona dzierżawa wraca tu i jest zwalniana (drop).
            drop(waiter.send(result));
        }
        drop(out.orphans);
        if !out.events.is_empty() {
            self.host.emit(out.events);
        }
        self.host.deadline_changed();
    }

    /// Zamienia efekty na wyniki/sygnały/zdarzenia. Zwraca wynik żądania `this`, jeśli rozstrzygnięty.
    fn apply(
        &self,
        engine: &mut Engine,
        effects: Vec<Effect>,
        this: Option<RequestId>,
        now: u64,
        out: &mut Outcome,
    ) -> Option<Result<Lease, SchedError>> {
        let mut ready = None;
        for effect in effects {
            out.events.push(effect_event(&effect, now));
            let (request, result) = match effect {
                Effect::Granted(info) => {
                    let (tx, rx) = watch::channel(LeaseSignal::Active);
                    engine.signals.insert(info.id, tx);
                    let request = info.request;
                    (request, Ok(Lease::new(info, self.control(), rx)))
                }
                Effect::TimedOut {
                    request,
                    resource,
                    on_timeout,
                } => {
                    let waited_ms = now.saturating_sub(request.enqueued_at_ms);
                    let err = SchedError::Timeout {
                        resource,
                        on_timeout,
                        waited_ms,
                    };
                    (request.id, Err(err))
                }
                Effect::Deadlock {
                    request,
                    resource,
                    cycle,
                } => (request.id, Err(SchedError::Deadlock { resource, cycle })),
                Effect::Cancelled { request, .. } => (request.id, Err(SchedError::Cancelled)),
                Effect::PreemptRequested { lease, by, reason } => {
                    if let Some(tx) = engine.signals.get(&lease.id) {
                        tx.send_replace(LeaseSignal::PreemptRequested { by, reason });
                    }
                    continue;
                }
                Effect::Revoked(lease, reason) => {
                    if let Some(tx) = engine.signals.remove(&lease.id) {
                        tx.send_replace(LeaseSignal::Revoked { reason });
                    }
                    continue;
                }
                Effect::HandedOff { lease, .. } => {
                    if let Some(tx) = engine.signals.remove(&lease.id) {
                        tx.send_replace(LeaseSignal::Revoked {
                            reason: PreemptReason::Handoff,
                        });
                    }
                    continue;
                }
                Effect::Released(lease) => {
                    engine.signals.remove(&lease.id);
                    continue;
                }
                Effect::Queued { .. } => continue,
            };
            if Some(request) == this {
                ready = Some(result);
            } else if let Some(waiter) = engine.waiters.remove(&request) {
                out.deliveries.push((waiter, result));
            } else if let Ok(lease) = result {
                out.orphans.push(lease);
            }
        }
        ready
    }
}

impl<H: Host> LeaseControl for Core<H> {
    fn release(&self, lease: LeaseId) {
        let _ = self.run(|table, now| Ok(table.release(lease, now)));
    }

    fn handoff(&self, lease: LeaseId, to: Holder) -> Result<(), SchedError> {
        self.run(|table, now| table.handoff(lease, to, now))
    }

    fn cancel(&self, request: RequestId) {
        let now = self.host.now_ms();
        let mut out = Outcome::default();
        {
            let mut engine = self.lock();
            engine.waiters.remove(&request);
            let effects = engine.table.cancel(request, now);
            self.apply(&mut engine, effects, Some(request), now, &mut out);
        }
        self.finish(out);
    }
}
