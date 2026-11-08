//! Mosty zdarzeń i stan otoczenia schedulera: `scheduler.task.*` → `TaskUpdated`,
//! `triggers.*` → `TriggerFired`, raport dzienny Marszałka → `MarshalReportReady`
//! (powiadomienie), eskalacje → toast, „nie przeszkadzać" z `voice-wake` → wyzwalacze, warunki
//! systemowe (bezczynność, tryb gry) → scheduler, obsada → `Roster::from_cast`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use app_api::EventHub;
use app_api::dto::{AlfaEvent, LocalizedText, ToastKind};
use core_bus_contract::{BusItem, EventBus, EventFilter};
use futures_util::StreamExt;
use marshal_contract::{DailyReport, EVENT_ESCALATION, EVENT_REPORT, Escalation};
use personas_contract::Cast;
use scheduler_contract::{Roster, Scheduler, SystemConditions, TaskId};
use triggers_contract::{TriggerId, Triggers};

use crate::app::TasksApp;
use crate::map;

/// Obsada schedulera: obsada domyślna + limit równoległości (Ustawienia ∩ polityka Marszałka).
pub struct RosterCtl {
    cast: Mutex<Cast>,
    configured: AtomicU32,
    policy: AtomicU32,
}

impl RosterCtl {
    /// Obsada i limit z Ustawień.
    pub fn new(cast: Cast, max_parallel: u32) -> Self {
        Self {
            cast: Mutex::new(cast),
            configured: AtomicU32::new(max_parallel.max(1)),
            policy: AtomicU32::new(u32::MAX),
        }
    }

    /// Limit z polityki Marszałka (tylko zawęża).
    pub fn set_policy(&self, max_parallel: u32) {
        self.policy.store(max_parallel.max(1), Ordering::SeqCst);
    }

    /// Nowa obsada.
    pub fn set_cast(&self, cast: Cast) {
        *self.cast.lock().unwrap_or_else(PoisonError::into_inner) = cast;
    }

    /// Obsada dla schedulera.
    pub fn roster(&self) -> Roster {
        let n = self
            .configured
            .load(Ordering::SeqCst)
            .min(self.policy.load(Ordering::SeqCst))
            .max(1);
        let cast = self.cast.lock().unwrap_or_else(PoisonError::into_inner);
        Roster::from_cast(&cast, n)
    }

    /// Ustawia obsadę w schedulerze.
    pub fn apply(&self, scheduler: &dyn Scheduler) {
        scheduler.set_roster(self.roster());
    }
}

const TRIGGER_RUN_EVENTS: [&str; 4] = [
    triggers_contract::EVENT_FIRED,
    triggers_contract::EVENT_SUPPRESSED,
    triggers_contract::EVENT_DEFERRED,
    triggers_contract::EVENT_FAILED,
];

/// Zdarzenia modułów → `alfa://events`.
pub async fn spawn_bus_bridge(bus: Arc<dyn EventBus>, app: Arc<TasksApp>, events: EventHub) {
    let filters = [
        "scheduler.task.",
        "triggers.",
        "marshal.",
        voice_wake_contract::EVENT_DND,
    ];
    for prefix in filters {
        let Ok(mut stream) = bus.subscribe(EventFilter::prefix(prefix)).await else {
            tracing::warn!(prefiks = prefix, "most zdarzeń zadań niedostępny");
            continue;
        };
        let (app, events) = (app.clone(), events.clone());
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                let BusItem::Event(event) = item else {
                    continue;
                };
                handle(&app, &events, event.kind.as_str(), &event.payload);
            }
        });
    }
}

fn handle(app: &TasksApp, events: &EventHub, kind: &str, payload: &serde_json::Value) {
    if kind.starts_with("scheduler.task.") {
        if let Some(view) = payload["task"]
            .as_str()
            .and_then(|t| app.scheduler().task(&TaskId::new(t)))
        {
            events.emit(AlfaEvent::TaskUpdated {
                task: map::task(&view),
            });
        }
    } else if TRIGGER_RUN_EVENTS.contains(&kind) {
        let (Some(t), Some(triggers)) = (payload["trigger"].as_str(), app.triggers()) else {
            return;
        };
        if let Some(run) = triggers.log(Some(&TriggerId::new(t)), 1).last() {
            events.emit(AlfaEvent::TriggerFired { run: map::run(run) });
        }
    } else if kind == EVENT_REPORT {
        if let Ok(report) = serde_json::from_value::<DailyReport>(payload.clone()) {
            events.emit(AlfaEvent::MarshalReportReady {
                report: map::report(&report),
            });
        }
    } else if kind == EVENT_ESCALATION {
        if let Ok(e) = serde_json::from_value::<Escalation>(payload.clone()) {
            events.emit(AlfaEvent::Toast {
                kind: ToastKind::Warning,
                message: LocalizedText::new(format!("Marszałek: {}", e.message), e.message.clone()),
            });
        }
    } else if kind == voice_wake_contract::EVENT_DND
        && let (Some(on), Some(triggers)) = (payload["on"].as_bool(), app.triggers())
    {
        triggers.set_dnd(on);
    }
}

/// Warunki systemowe dla okien zadań (bezczynność z portu platformy, tryb gry) co `every`.
pub fn spawn_conditions(
    scheduler: Arc<dyn Scheduler>,
    probe: Arc<dyn Fn() -> SystemConditions + Send + Sync>,
    every: Duration,
) {
    tokio::spawn(async move {
        let mut last = None;
        let mut ticker = tokio::time::interval(every);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let now = probe();
            if last != Some(now) {
                scheduler.set_conditions(now);
                last = Some(now);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use personas_contract::{PersonaId, RoleId};

    #[test]
    fn roster_limit_is_the_narrower_of_settings_and_policy() {
        let cast = Cast::solo(PersonaId::gama(), [RoleId::critic()], false);
        let ctl = RosterCtl::new(cast, 4);
        assert_eq!(ctl.roster().max_parallel_total, 4);
        ctl.set_policy(2);
        assert_eq!(ctl.roster().max_parallel_total, 2);
        ctl.set_policy(9);
        assert_eq!(ctl.roster().max_parallel_total, 4);
        assert_eq!(ctl.roster().agents.len(), 1);
    }
}
