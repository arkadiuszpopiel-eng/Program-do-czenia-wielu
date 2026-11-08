//! Deterministyczna magistrala do testów (docs/PLAN.md §4.5, SPEC core-bus „Fake”).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod clock;

use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::{
    BusError, BusItem, BusStats, Event, EventBus, EventFilter, EventKind, EventStream,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

pub use clock::VirtualClock;

/// Błędy specyficzne dla atrapy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FakeBusError {
    /// Indeks spoza zakresu nagranych zdarzeń.
    #[error("indeks {index} poza zakresem nagrania ({len} zdarzeń)")]
    IndexOutOfRange {
        /// Żądany indeks.
        index: usize,
        /// Liczba nagranych zdarzeń.
        len: usize,
    },
}

struct Subscriber {
    filter: EventFilter,
    tx: mpsc::UnboundedSender<BusItem>,
}

#[derive(Default)]
struct State {
    recorded: Vec<Arc<Event>>,
    subscribers: Vec<Subscriber>,
    next_id: u128,
}

/// Magistrala-atrapa: synchroniczne dostarczanie, pełne nagranie, deterministyczne `id`/`ts`.
#[derive(Clone)]
pub struct FakeBus {
    state: Arc<Mutex<State>>,
    clock: VirtualClock,
    stamp: bool,
}

impl Default for FakeBus {
    fn default() -> Self {
        Self::new(VirtualClock::default())
    }
}

impl FakeBus {
    /// Atrapa z podanym zegarem; `id` i `ts` zdarzeń są nadpisywane deterministycznie.
    pub fn new(clock: VirtualClock) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            clock,
            stamp: true,
        }
    }

    /// Wyłącza nadpisywanie `id`/`ts` (zdarzenia przechodzą bez zmian).
    #[must_use]
    pub fn without_stamping(mut self) -> Self {
        self.stamp = false;
        self
    }

    /// Wirtualny zegar magistrali.
    pub fn clock(&self) -> &VirtualClock {
        &self.clock
    }

    /// Wszystkie nagrane zdarzenia w kolejności publikacji.
    pub fn recorded(&self) -> Vec<Arc<Event>> {
        self.lock().recorded.clone()
    }

    /// Nagrane zdarzenia danego rodzaju.
    pub fn recorded_of_kind(&self, kind: &EventKind) -> Vec<Arc<Event>> {
        self.lock()
            .recorded
            .iter()
            .filter(|ev| &ev.kind == kind)
            .cloned()
            .collect()
    }

    /// Czyści nagranie (subskrypcje zostają).
    pub fn clear(&self) {
        self.lock().recorded.clear();
    }

    /// Odtwarza nagrane zdarzenia do bieżących subskrybentów w zadanej kolejności indeksów.
    pub fn replay(&self, order: impl IntoIterator<Item = usize>) -> Result<usize, FakeBusError> {
        let mut state = self.lock();
        let len = state.recorded.len();
        let mut delivered = 0;
        for index in order {
            let ev = state
                .recorded
                .get(index)
                .cloned()
                .ok_or(FakeBusError::IndexOutOfRange { index, len })?;
            deliver(&mut state, &ev);
            delivered += 1;
        }
        Ok(delivered)
    }

    /// Odtwarza całe nagranie w oryginalnej kolejności.
    pub fn replay_all(&self) -> usize {
        let len = self.lock().recorded.len();
        self.replay(0..len).unwrap_or(0)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn deliver(state: &mut State, ev: &Arc<Event>) {
    state.subscribers.retain(|sub| {
        if !sub.filter.matches(ev) {
            return true;
        }
        sub.tx.send(BusItem::Event(Arc::clone(ev))).is_ok()
    });
}

#[async_trait]
impl EventBus for FakeBus {
    async fn publish(&self, mut event: Event) -> Result<(), BusError> {
        let mut state = self.lock();
        if self.stamp {
            state.next_id += 1;
            event.id = Uuid::from_u128(state.next_id);
            event.ts = self.clock.now();
        }
        let ev = Arc::new(event);
        state.recorded.push(Arc::clone(&ev));
        deliver(&mut state, &ev);
        Ok(())
    }

    async fn subscribe(&self, filter: EventFilter) -> Result<EventStream, BusError> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.lock().subscribers.push(Subscriber { filter, tx });
        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }

    fn stats(&self) -> BusStats {
        let state = self.lock();
        BusStats {
            published: state.recorded.len() as u64,
            dropped: 0,
            subscribers: state
                .subscribers
                .iter()
                .filter(|s| !s.tx.is_closed())
                .count(),
        }
    }
}
