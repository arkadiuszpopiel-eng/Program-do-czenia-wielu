//! Po starcie: dokończenie przerwanych usunięć (crypto-shredding sesji z kosza), kurs NBP w tle,
//! przywrócenie zapisanego (niższego) poziomu autonomii w Brokerze, aktualizacje w tle
//! (`updater::mark_good` po zdrowym starcie, sprawdzanie wg trybu) i mosty zdarzeń modułów → `alfa://events` (limit kosztów, postęp pobierania
//! modelu lokalnego).

use std::time::Duration;

use core_bus_contract::{BusItem, EventFilter};
use cost_meter_contract::{CostMeter, EVENT_LIMIT_BLOCKED, EVENT_LIMIT_WARNING};
use futures_util::StreamExt;
use sessions_contract::{SessionCatalog, SessionQuery};

use crate::core::AppCore;
use crate::dto::{AlfaEvent, LocalDownloadState, LocalizedText, ToastKind};
use crate::events::EventHub;

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
        self.restore_autonomy().await;
        self.inner
            .voice
            .attach(std::sync::Arc::new(crate::voice_chat::CoreVoiceChat::new(
                self,
            )));
        self.spawn_bus_bridge().await;
        self.spawn_download_bridge().await;
        app_memory::spawn_bridge(self.inner.bus.clone(), self.inner.events.clone()).await;
        self.spawn_task_bridges().await;
        self.spawn_updates(self.inner.healthy_after);
        self.inner.work.files.start();
    }

    /// Zadania: zdarzenia `scheduler.*`/`triggers.*`/`marshal.*` → UI, DND z `voice-wake` →
    /// wyzwalacze, warunki okien zadań co 5 s (bezczynność i blokada z monitora sygnałów, tryb
    /// gry z monitora albo `model-residency`; bez monitora — „nigdy bezczynny") i cykl
    /// Ulepszacza w bezczynności (tylko z monitorem; bez niego — „Przeanalizuj teraz").
    async fn spawn_task_bridges(&self) {
        let tasks = self.inner.tasks.clone();
        app_tasks::spawn_bus_bridge(
            self.inner.bus.clone(),
            tasks.clone(),
            self.inner.events.clone(),
        )
        .await;
        let residency = self
            .inner
            .extra
            .residency
            .as_ref()
            .map(|r| r.manager() as std::sync::Arc<dyn model_residency_contract::Residency>);
        let signals = self.inner.signals.clone();
        let probe = std::sync::Arc::new(move || {
            let s = crate::parts::signals::snapshot(signals.as_ref());
            scheduler_contract::SystemConditions {
                user_idle: s.user_idle || s.locked(),
                game_mode: s.game_mode()
                    || residency.as_ref().is_some_and(|r| r.snapshot().mode.gaming),
            }
        });
        app_tasks::spawn_conditions(tasks.scheduler(), probe, Duration::from_secs(5));
        if let Some(port) = self.inner.signals.clone() {
            let conditions = std::sync::Arc::new(move || {
                let s = port.snapshot();
                app_health::RunConditions {
                    on_battery: s.on_battery(),
                    game_mode: s.game_mode(),
                    user_idle: s.user_idle || s.locked(),
                }
            });
            let health = &self.inner.work.health;
            health.spawn_idle_cycle(conditions, app_health::IDLE_CYCLE_EVERY);
        }
    }

    /// Aktualizacje w tle (`app-updates`): `mark_good` po `after` bez awarii, sprawdzanie wg
    /// trybu; restart blokuje trwające zadanie agentki albo rozmowa głosowa.
    fn spawn_updates(&self, after: Duration) {
        let weak = std::sync::Arc::downgrade(&self.inner);
        let updates = &self.inner.work.updates;
        updates.set_busy_probe(std::sync::Arc::new(move || {
            let inner = weak.upgrade();
            Box::pin(async move {
                let inner = inner?;
                let task = inner.chat.has_runs();
                let voice = inner.voice.status().await.state == crate::dto::VoiceState::Active;
                let (pl, en) = match (task, voice) {
                    (true, _) => (
                        "Agentka wykonuje zadanie — dokończ je albo zatrzymaj.",
                        "An agent is working — finish or stop the task first.",
                    ),
                    (_, true) => (
                        "Trwa rozmowa głosowa — zakończ ją najpierw.",
                        "A voice conversation is active — end it first.",
                    ),
                    _ => return None,
                };
                Some(crate::dto::LocalizedText::new(pl, en))
            })
        }));
        let schedule = app_updates::Schedule {
            healthy_after: after,
            ..Default::default()
        };
        updates.spawn_background(schedule);
    }

    /// `local.model.download.progress` → `LocalModelProgress { state: downloading }`.
    async fn spawn_download_bridge(&self) {
        let stream = self
            .inner
            .bus
            .subscribe(EventFilter::prefix(
                providers_local_impl::EVENT_DOWNLOAD_PROGRESS,
            ))
            .await;
        let Ok(mut stream) = stream else {
            tracing::warn!("most postępu pobierania modelu niedostępny");
            return;
        };
        let events: EventHub = self.inner.events.clone();
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                let BusItem::Event(event) = item else {
                    continue;
                };
                let p = &event.payload;
                let (Some(model), Some(bytes)) =
                    (p["model"].as_str(), p["progress"]["bytes"].as_u64())
                else {
                    continue;
                };
                events.emit(AlfaEvent::LocalModelProgress {
                    model_id: model.to_owned(),
                    state: LocalDownloadState::Downloading,
                    bytes,
                    total: p["progress"]["total"].as_u64(),
                    error: None,
                });
            }
        });
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
