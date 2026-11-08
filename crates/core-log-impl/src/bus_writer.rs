//! Log-writer podpięty do magistrali: zdarzenia → strumienie `LogSink`, Audyt → `AuditWriter`.

use std::sync::Arc;

use core_bus_contract::{BusError, BusItem, EventBus, EventFilter, EventKind};
use core_log_contract::{AuditWriter, LogSink, LogStream};
use futures_util::StreamExt;
use tokio::task::JoinHandle;

/// Dokąd trafia zdarzenie danego rodzaju.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Strumień `LogSink`.
    Stream(LogStream),
    /// Łańcuch Audytu (pre-broker; od F3 — Broker).
    Audit,
    /// Nie zapisujemy (zdarzenia UI).
    Skip,
}

/// Trasa zdarzenia: rodzaje wbudowane wg `LogStream::for_kind`, `audit` → Audyt,
/// zdarzenia własne modułów (`<moduł>.<obiekt>.<czynność>`) → Diagnostyka, `ui` → pominięte.
pub fn route(kind: &EventKind) -> Route {
    match (kind, LogStream::for_kind(kind)) {
        (_, Some(stream)) => Route::Stream(stream),
        (EventKind::Audit, None) => Route::Audit,
        (EventKind::Custom(_), None) => Route::Stream(LogStream::Diagnostics),
        _ => Route::Skip,
    }
}

/// Subskrybuje całą magistralę i zapisuje zdarzenia wg `route`. Audyt bez writera jest
/// pomijany z ostrzeżeniem (nigdy po cichu). Zadanie kończy się wraz z zamknięciem magistrali.
pub async fn spawn_bus_writer(
    bus: Arc<dyn EventBus>,
    sink: Arc<dyn LogSink>,
    audit: Option<Arc<dyn AuditWriter>>,
) -> Result<JoinHandle<()>, BusError> {
    let mut events = bus.subscribe(EventFilter::all()).await?;
    Ok(tokio::spawn(async move {
        while let Some(item) = events.next().await {
            let event = match item {
                BusItem::Event(event) => event,
                BusItem::Lagged(n) => {
                    tracing::warn!(pominiete = n, "log-writer nie nadążył za magistralą");
                    continue;
                }
            };
            let result = match (route(&event.kind), &audit) {
                (Route::Stream(stream), _) => sink.append(stream, &event).await.map(|_| ()),
                (Route::Audit, Some(writer)) => writer.append_audit(&event).await.map(|_| ()),
                (Route::Audit, None) => {
                    tracing::warn!("zdarzenie audytu bez writera Audytu");
                    Ok(())
                }
                (Route::Skip, _) => Ok(()),
            };
            if let Err(e) = result {
                tracing::error!(error = %e, rodzaj = %event.kind, "zapis logu nie powiódł się");
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes() {
        assert_eq!(route(&EventKind::Gui), Route::Stream(LogStream::ToolsGui));
        assert_eq!(route(&EventKind::Audit), Route::Audit);
        assert_eq!(
            route(&EventKind::Custom("registry.module.health".into())),
            Route::Stream(LogStream::Diagnostics)
        );
        assert_eq!(route(&EventKind::Ui), Route::Skip);
    }
}
