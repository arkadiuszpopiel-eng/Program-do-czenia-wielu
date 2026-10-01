//! Po starcie: dokończenie przerwanych usunięć (crypto-shredding sesji z kosza), kurs NBP w tle
//! i most zdarzeń modułów → `alfa://events` (komunikaty limitu kosztów).

use core_bus_contract::{BusItem, EventFilter};
use cost_meter_contract::{CostMeter, EVENT_LIMIT_BLOCKED, EVENT_LIMIT_WARNING};
use futures_util::StreamExt;
use sessions_contract::{SessionCatalog, SessionQuery};

use crate::core::AppCore;
use crate::dto::{AlfaEvent, LocalizedText, ToastKind};

impl AppCore {
    /// Zadania po złożeniu modułów.
    pub(crate) async fn after_start(&self) {
        let trashed = self.inner.sessions.list_sessions(&SessionQuery {
            trashed: true,
            include_archived: true,
            ..SessionQuery::default()
        });
        for s in trashed.unwrap_or_default() {
            // Okno cofnięcia nie przetrwało restartu — usunięcie ostateczne (PLAN §14.8).
            self.shred_session(&s.meta.id);
        }
        let costs = self.inner.costs.clone();
        tokio::spawn(async move {
            costs.refresh_fx().await;
        });
        self.spawn_bus_bridge().await;
    }

    async fn spawn_bus_bridge(&self) {
        let stream = self
            .inner
            .bus
            .subscribe(EventFilter::prefix("cost.limit."))
            .await;
        let Ok(mut stream) = stream else {
            tracing::warn!("most zdarzeń kosztów niedostępny");
            return;
        };
        let events = self.inner.events.clone();
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                let BusItem::Event(event) = item else {
                    continue;
                };
                let (kind, pl, en) = match event.kind.as_str() {
                    EVENT_LIMIT_BLOCKED => (
                        ToastKind::Error,
                        "Limit kosztów osiągnięty — kolejne zapytania płatne są wstrzymane.",
                        "Cost limit reached — paid requests are paused.",
                    ),
                    EVENT_LIMIT_WARNING => (
                        ToastKind::Warning,
                        "Zbliżasz się do miesięcznego limitu kosztów.",
                        "You are approaching the monthly cost limit.",
                    ),
                    _ => continue,
                };
                events.emit(AlfaEvent::Toast {
                    kind,
                    message: LocalizedText::new(pl, en),
                });
            }
        });
    }
}
