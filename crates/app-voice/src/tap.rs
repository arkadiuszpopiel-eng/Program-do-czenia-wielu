//! Magistrala potoku: zdarzenia `voice.pipeline.*` trafiają do pętli trybu głosowego (pigułka,
//! transkrypt częściowy, „stop wszystko", „anuluj", degradacje), a zdarzenia od poziomu `Info`
//! także na magistralę aplikacji (logi). Pigułka (poziom `Trace`, co 100 ms) nie zaśmieca logów.

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::{BusError, BusStats, Event, EventBus, EventFilter, EventStream, Level};
use tokio::sync::mpsc;

/// Magistrala-podsłuch potoku.
pub struct TapBus {
    tx: mpsc::UnboundedSender<Event>,
    inner: Option<Arc<dyn EventBus>>,
}

impl TapBus {
    /// Podsłuch z kanałem do pętli i opcjonalną magistralą aplikacji.
    pub fn new(inner: Option<Arc<dyn EventBus>>) -> (Self, mpsc::UnboundedReceiver<Event>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx, inner }, rx)
    }
}

#[async_trait]
impl EventBus for TapBus {
    async fn publish(&self, event: Event) -> Result<(), BusError> {
        if let Some(bus) = &self.inner
            && event.level >= Level::Info
        {
            // Magistrala aplikacji jest best effort — pętla głosu i tak dostaje zdarzenie.
            let _ = bus.publish(event.clone()).await;
        }
        // Zamknięty kanał = pętla już się zakończyła; zdarzenie nie ma adresata.
        let _ = self.tx.send(event);
        Ok(())
    }

    async fn subscribe(&self, filter: EventFilter) -> Result<EventStream, BusError> {
        match &self.inner {
            Some(bus) => bus.subscribe(filter).await,
            None => Err(BusError::Closed),
        }
    }

    fn stats(&self) -> BusStats {
        self.inner.as_ref().map(|b| b.stats()).unwrap_or_default()
    }
}
