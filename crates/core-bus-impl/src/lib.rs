//! Magistrala zdarzeń in-proc na `tokio::sync::broadcast` (docs/modules/core-bus/SPEC.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use async_trait::async_trait;
use core_bus_contract::{BusError, BusItem, BusStats, Event, EventBus, EventFilter, EventStream};
use futures_util::StreamExt;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

/// Konfiguracja magistrali (klucze `[core.bus]` z SPEC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusConfig {
    /// Pojemność bufora per subskrybent; po przepełnieniu najstarsze zdarzenia są gubione.
    pub buffer_per_subscriber: usize,
    /// Maksymalny rozmiar ładunku inline w bajtach (większe → `BusError::PayloadTooLarge`).
    pub max_inline_payload_bytes: usize,
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            buffer_per_subscriber: 4096,
            max_inline_payload_bytes: 64 * 1024,
        }
    }
}

#[derive(Default)]
struct Counters {
    published: AtomicU64,
    dropped: AtomicU64,
    subscribers: AtomicUsize,
}

/// Magistrala na kanale `broadcast`: jeden nadawca, wiele odbiorców, bufor ograniczony.
pub struct BroadcastBus {
    tx: broadcast::Sender<Arc<Event>>,
    config: BusConfig,
    counters: Arc<Counters>,
}

impl BroadcastBus {
    /// Tworzy magistralę z podaną konfiguracją (bufor < 1 jest podnoszony do 1).
    pub fn new(config: BusConfig) -> Self {
        let capacity = config.buffer_per_subscriber.max(1);
        let (tx, _rx) = broadcast::channel(capacity);
        Self {
            tx,
            config,
            counters: Arc::new(Counters::default()),
        }
    }

    /// Konfiguracja użyta przy tworzeniu.
    pub fn config(&self) -> BusConfig {
        self.config
    }

    fn check_payload(&self, event: &Event) -> Result<(), BusError> {
        let size = payload_size(&event.payload);
        let limit = self.config.max_inline_payload_bytes;
        if size > limit {
            return Err(BusError::PayloadTooLarge { size, limit });
        }
        Ok(())
    }
}

impl Default for BroadcastBus {
    fn default() -> Self {
        Self::new(BusConfig::default())
    }
}

/// Przybliżony rozmiar ładunku (długość zserializowanego JSON).
fn payload_size(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Null => 0,
        other => other.to_string().len(),
    }
}

/// Uchwyt liczący aktywne subskrypcje — zwalniany razem ze strumieniem.
struct SubscriptionGuard(Arc<Counters>);

impl Drop for SubscriptionGuard {
    fn drop(&mut self) {
        self.0.subscribers.fetch_sub(1, Ordering::Relaxed);
    }
}

#[async_trait]
impl EventBus for BroadcastBus {
    async fn publish(&self, event: Event) -> Result<(), BusError> {
        self.check_payload(&event)?;
        // `send` nie blokuje: brak odbiorców to nie błąd, przepełnienie gubi najstarsze wpisy
        // u odbiorcy (raportowane jako `Lagged` po jego stronie).
        let _ = self.tx.send(Arc::new(event));
        self.counters.published.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn subscribe(&self, filter: EventFilter) -> Result<EventStream, BusError> {
        let rx = self.tx.subscribe();
        self.counters.subscribers.fetch_add(1, Ordering::Relaxed);
        let guard = Arc::new(SubscriptionGuard(Arc::clone(&self.counters)));
        let counters = Arc::clone(&self.counters);
        let stream = BroadcastStream::new(rx).filter_map(move |item| {
            let _keep_alive = Arc::clone(&guard);
            let mapped = match item {
                Ok(ev) if filter.matches(&ev) => Some(BusItem::Event(ev)),
                Ok(_) => None,
                Err(BroadcastStreamRecvError::Lagged(n)) => {
                    counters.dropped.fetch_add(n, Ordering::Relaxed);
                    tracing::warn!(skipped = n, "subskrybent magistrali nie nadąża");
                    Some(BusItem::Lagged(n))
                }
            };
            std::future::ready(mapped)
        });
        Ok(Box::pin(stream))
    }

    fn stats(&self) -> BusStats {
        BusStats {
            published: self.counters.published.load(Ordering::Relaxed),
            dropped: self.counters.dropped.load(Ordering::Relaxed),
            subscribers: self.counters.subscribers.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_buffer_is_clamped() {
        let bus = BroadcastBus::new(BusConfig {
            buffer_per_subscriber: 0,
            ..BusConfig::default()
        });
        assert_eq!(bus.config().buffer_per_subscriber, 0);
        assert_eq!(bus.stats(), BusStats::default());
    }
}
