//! Trait `EventBus` i typy pomocnicze strumienia.

use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures_core::Stream;

use crate::event::Event;
use crate::filter::EventFilter;

/// Element strumienia subskrypcji.
#[derive(Debug, Clone)]
pub enum BusItem {
    /// Zdarzenie spełniające filtr subskrypcji.
    Event(Arc<Event>),
    /// Subskrybent nie nadążył; tyle zdarzeń zostało pominiętych (SPEC: `bus.subscriber_lagged`).
    Lagged(u64),
}

impl BusItem {
    /// Zwraca zdarzenie, jeśli element nim jest.
    pub fn event(&self) -> Option<&Arc<Event>> {
        match self {
            BusItem::Event(ev) => Some(ev),
            BusItem::Lagged(_) => None,
        }
    }
}

/// Strumień subskrypcji (async, `Send`).
pub type EventStream = Pin<Box<dyn Stream<Item = BusItem> + Send>>;

/// Błędy magistrali.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum BusError {
    /// Magistrala została zamknięta.
    #[error("magistrala zamknięta")]
    Closed,
    /// Zdarzenie odrzucone przez walidację schematu (tryb `strict_schema`).
    #[error("zdarzenie odrzucone przez schemat: {0}")]
    SchemaRejected(String),
    /// Ładunek przekracza limit inline (SPEC: `max_inline_payload_bytes`).
    #[error("ładunek za duży: {size} B > {limit} B")]
    PayloadTooLarge {
        /// Rozmiar ładunku.
        size: usize,
        /// Obowiązujący limit.
        limit: usize,
    },
}

/// Liczniki magistrali (do diagnostyki i testów backpressure).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BusStats {
    /// Liczba opublikowanych zdarzeń.
    pub published: u64,
    /// Liczba zdarzeń zgubionych przez wolnych subskrybentów (suma po subskrypcjach).
    pub dropped: u64,
    /// Liczba aktywnych subskrypcji.
    pub subscribers: usize,
}

/// Magistrala zdarzeń. `publish` nigdy nie blokuje wydawcy: wolny subskrybent dostaje
/// `BusItem::Lagged`, a licznik `dropped` rośnie (backpressure bez blokady).
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publikuje zdarzenie do wszystkich pasujących subskrypcji.
    async fn publish(&self, event: Event) -> Result<(), BusError>;

    /// Otwiera subskrypcję; strumień zwraca tylko zdarzenia spełniające filtr.
    async fn subscribe(&self, filter: EventFilter) -> Result<EventStream, BusError>;

    /// Bieżące liczniki.
    fn stats(&self) -> BusStats;
}
