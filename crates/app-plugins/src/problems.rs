//! Problemy wtyczek dla Diagnosty i strony „Wtyczki”: magistrala runtime'u jest opakowana
//! dekoratorem, który zapamiętuje `plugin.trapped` (piaskownica przerwała wywołanie) i
//! `plugin.load_failed` (moduł odrzucony przy ładowaniu) — bez treści wejścia/wyjścia — i przekazuje
//! wszystko dalej. Zdrowie modułu `plugin-runtime` jest `Degraded`, póki ostatni problem jest
//! świeży; rejestr publikuje to jako `registry.module.health`, a Diagnosta przyjmuje sygnał stanu
//! modułu (karta w „Zdrowiu systemu”).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use app_api::dto::{PluginProblem, PluginProblemKind};
use async_trait::async_trait;
use core_bus_contract::{BusError, BusStats, Event, EventBus, EventFilter, EventStream};
use core_registry_contract::HealthStatus;
use plugin_runtime_contract::events;

/// Ile problemów pamiętamy.
pub const MAX_PROBLEMS: usize = 20;
/// Jak długo problem obniża zdrowie modułu.
pub const FRESH_FOR: Duration = Duration::from_secs(15 * 60);

/// Ostatnie problemy (najnowszy na początku).
#[derive(Debug, Default)]
pub struct Problems {
    list: Mutex<VecDeque<(PluginProblem, chrono::DateTime<chrono::Utc>)>>,
}

impl Problems {
    fn lock(&self) -> MutexGuard<'_, VecDeque<(PluginProblem, chrono::DateTime<chrono::Utc>)>> {
        self.list.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zapamiętuje problem ze zdarzenia (inne zdarzenia — bez zmian).
    pub fn observe(&self, event: &Event) {
        let kind = match event.kind.as_str() {
            events::TRAPPED => PluginProblemKind::Trapped,
            events::LOAD_FAILED => PluginProblemKind::LoadFailed,
            _ => return,
        };
        let p = &event.payload;
        let text = |k: &str| {
            p.get(k)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let detail = match kind {
            PluginProblemKind::Trapped => format!("{}: {}", text("tool"), text("result")),
            PluginProblemKind::LoadFailed => p
                .get("error")
                .map(|e| match e.get("load_error") {
                    Some(k) => k.as_str().unwrap_or_default().to_owned(),
                    None => e.to_string(),
                })
                .unwrap_or_default(),
        };
        let problem = PluginProblem {
            plugin: text("plugin"),
            version: text("version"),
            kind,
            detail: detail.chars().take(200).collect(),
            at: app_api::dto::iso(event.ts),
        };
        let mut list = self.lock();
        list.push_front((problem, event.ts));
        list.truncate(MAX_PROBLEMS);
    }

    /// Lista (najnowszy pierwszy).
    pub fn list(&self) -> Vec<PluginProblem> {
        self.lock().iter().map(|(p, _)| p.clone()).collect()
    }

    /// Zdrowie: `Degraded` z opisem ostatniego świeżego problemu.
    pub fn health(&self, now: chrono::DateTime<chrono::Utc>) -> HealthStatus {
        let fresh = chrono::Duration::from_std(FRESH_FOR).unwrap_or(chrono::Duration::zero());
        match self.lock().front() {
            Some((p, at)) if now.signed_duration_since(*at) < fresh => {
                let what = match p.kind {
                    PluginProblemKind::Trapped => "przerwana przez piaskownicę",
                    PluginProblemKind::LoadFailed => "moduł odrzucony przy ładowaniu",
                };
                HealthStatus::Degraded(format!(
                    "wtyczka {} {}: {} ({})",
                    p.plugin, p.version, what, p.detail
                ))
            }
            _ => HealthStatus::Healthy,
        }
    }
}

/// Magistrala runtime'u wtyczek: zapamiętuje problemy i przekazuje zdarzenia dalej.
pub struct ProblemBus {
    inner: Option<Arc<dyn EventBus>>,
    problems: Arc<Problems>,
}

impl ProblemBus {
    /// Dekorator nad magistralą aplikacji (`None` — tylko zapamiętywanie).
    pub fn new(inner: Option<Arc<dyn EventBus>>, problems: Arc<Problems>) -> Self {
        Self { inner, problems }
    }
}

#[async_trait]
impl EventBus for ProblemBus {
    async fn publish(&self, event: Event) -> Result<(), BusError> {
        self.problems.observe(&event);
        match &self.inner {
            Some(bus) => bus.publish(event).await,
            None => Ok(()),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use core_bus_contract::{EventKind, Level};

    fn ev(kind: &str, payload: serde_json::Value) -> Event {
        Event::new(EventKind::Custom(kind.into()), Level::Warn, payload)
    }

    #[tokio::test]
    async fn trapped_and_load_failed_degrade_health_until_stale() {
        let problems = Arc::new(Problems::default());
        let bus = ProblemBus::new(None, problems.clone());
        let now = chrono::Utc::now();
        assert_eq!(problems.health(now), HealthStatus::Healthy);
        bus.publish(ev("plugin.invoked", serde_json::json!({"plugin": "a"})))
            .await
            .unwrap();
        assert!(problems.list().is_empty());
        bus.publish(ev(
            events::TRAPPED,
            serde_json::json!({"plugin": "licznik", "version": "1.0.0", "tool": "word_count", "result": "out_of_fuel"}),
        ))
        .await
        .unwrap();
        bus.publish(ev(
            events::LOAD_FAILED,
            serde_json::json!({"plugin": "zly", "version": "2.0.0", "error": {"load_error": "hash_mismatch", "detail": {}}}),
        ))
        .await
        .unwrap();
        let list = problems.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].kind, PluginProblemKind::LoadFailed);
        assert_eq!(list[0].detail, "hash_mismatch");
        assert_eq!(list[1].detail, "word_count: out_of_fuel");
        let HealthStatus::Degraded(why) = problems.health(chrono::Utc::now()) else {
            panic!("oczekiwano Degraded");
        };
        assert!(why.contains("zly") && why.contains("odrzucony"));
        let later = chrono::Utc::now() + chrono::Duration::hours(1);
        assert_eq!(problems.health(later), HealthStatus::Healthy);
        assert!(bus.subscribe(EventFilter::all()).await.is_err());
        assert_eq!(bus.stats(), BusStats::default());
    }

    #[tokio::test]
    async fn ring_is_bounded() {
        let problems = Problems::default();
        for i in 0..(MAX_PROBLEMS + 5) {
            problems.observe(&ev(
                events::TRAPPED,
                serde_json::json!({"plugin": format!("p{i}"), "result": "timeout"}),
            ));
        }
        assert_eq!(problems.list().len(), MAX_PROBLEMS);
    }
}
