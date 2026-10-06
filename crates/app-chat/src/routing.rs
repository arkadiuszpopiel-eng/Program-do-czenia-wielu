//! Decyzja Routera w rozmowie: który model faktycznie odpowiada (`Started` z `dostawca:model`)
//! → kapsuła aktywności i wpis osi czasu z uzasadnieniem (wybrany, zapasowe, odrzucone, fallback).

use app_api::dto::{self, ActivityInfo, AlfaEvent, EventLevel, TimelineEvent, TimelineKind};
use app_api::ids;
use app_api::ports::{BrainChoice, BrainTarget};

use crate::engine::ChatEngine;
use crate::generate::GenRequest;
use crate::outcome::Chosen;

/// Wybór dostawcy do kosztów i statystyk.
pub(crate) fn chosen_of(target: &BrainTarget) -> Chosen {
    Chosen {
        provider_id: target.provider_id.clone(),
        provider_name: target.provider_name.clone(),
        account: target.account.clone(),
        model: target.model.clone(),
    }
}

/// Uzasadnienie trasy (bez treści rozmowy).
fn detail(choice: &BrainChoice, actual: &str) -> Option<String> {
    let route = choice.route.as_ref()?;
    let mut parts = Vec::new();
    if route.chosen != actual {
        parts.push(format!("fallback z {}", route.chosen));
    }
    if !route.fallbacks.is_empty() {
        parts.push(format!("zapasowe: {}", route.fallbacks.join(", ")));
    }
    if !route.rejected.is_empty() {
        parts.push(format!("odrzucone: {}", route.rejected.join("; ")));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// Kapsuła aktywności („odpowiada: …") i oś czasu (`router.decision`).
pub(crate) fn announce(
    core: &ChatEngine,
    req: &GenRequest,
    turn: &str,
    choice: &BrainChoice,
    t: &BrainTarget,
) {
    let actual = format!("{}:{}", t.provider_id, t.model);
    let place = if t.local { "lokalnie" } else { "przez API" };
    let now = dto::iso(chrono::Utc::now());
    core.emit(AlfaEvent::ActivityChanged {
        session_id: req.session.to_string(),
        activity: Some(ActivityInfo {
            session_id: req.session.to_string(),
            agent: req.agent.clone(),
            description: format!("Odpowiada {} · {} ({place})", t.provider_name, t.model),
            step: 1,
            total_steps: 1,
            started_at: now.clone(),
        }),
    });
    let event = TimelineEvent {
        id: ids::timeline_dto(&req.session),
        ts: now,
        session_id: req.session.to_string(),
        kind: TimelineKind::ModelCall,
        level: EventLevel::Info,
        agent: Some(req.agent.clone()),
        title: format!("Router → {} · {}", t.provider_name, t.model),
        detail: detail(choice, &actual),
        cost: None,
        latency_ms: None,
        turn_id: Some(turn.to_owned()),
    };
    if let Err(e) = core.inner.store.push_timeline(&req.session, &event) {
        tracing::warn!(error = %e, "zapis decyzji Routera na osi czasu nie powiódł się");
    }
    core.emit(AlfaEvent::TimelineAppended { event });
}
