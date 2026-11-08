//! Kolejka zdarzeń na magistralę: `publish` jest asynchroniczne, więc zdarzenia czekają w kolejce
//! i są wysyłane po jednym przez przyszłość odpytywaną w kroku (bez blokowania wątku).

use std::collections::VecDeque;
use std::sync::Arc;
use std::task::Context;

use core_bus_contract::{Event, EventBus};

use crate::poll::{BoxFut, poll_slot};

/// Ile zdarzeń może czekać (najstarsze są porzucane, licznik `dropped`).
const MAX_QUEUE: usize = 4_096;

pub(crate) struct Outbox {
    bus: Option<Arc<dyn EventBus>>,
    queue: VecDeque<Event>,
    in_flight: Option<BoxFut<()>>,
    dropped: u64,
}

impl Outbox {
    pub(crate) fn new(bus: Option<Arc<dyn EventBus>>) -> Self {
        Self {
            bus,
            queue: VecDeque::new(),
            in_flight: None,
            dropped: 0,
        }
    }

    pub(crate) fn push(&mut self, event: Event) {
        if self.bus.is_none() {
            return;
        }
        if self.queue.len() >= MAX_QUEUE {
            self.queue.pop_front();
            self.dropped += 1;
        }
        self.queue.push_back(event);
    }

    /// Wysyła tyle, ile się da bez czekania; zwraca liczbę wysłanych.
    pub(crate) fn poll(&mut self, cx: &mut Context<'_>) -> u32 {
        let Some(bus) = self.bus.clone() else {
            return 0;
        };
        let mut sent = 0;
        loop {
            if self.in_flight.is_some() {
                if poll_slot(&mut self.in_flight, cx).is_none() {
                    break;
                }
                sent += 1;
                continue;
            }
            let Some(event) = self.queue.pop_front() else {
                break;
            };
            let bus = Arc::clone(&bus);
            self.in_flight = Some(Box::pin(async move {
                let _ = bus.publish(event).await;
            }));
        }
        sent
    }

    pub(crate) fn dropped(&self) -> u64 {
        self.dropped
    }
}
