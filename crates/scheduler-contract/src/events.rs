//! Zdarzenia magistrali `scheduler.task.*` (Oś czasu, kapsuła aktywności, Marszałek).
//! Ładunki zawierają identyfikatory, liczniki, tytuł (skrócony) i powody — nigdy ładunku zadania
//! ani treści sterowania.

use core_bus_contract::{AgentId, Event, EventKind, Level};
use serde_json::{Value, json};

use crate::ids::TaskId;

/// Zgłoszono zadanie.
pub const EVENT_SUBMITTED: &str = "scheduler.task.submitted";
/// Zmienił się powód czekania gotowego zadania.
pub const EVENT_BLOCKED: &str = "scheduler.task.blocked";
/// Wysłano do wykonawczyni (przydział agentki i zasobów).
pub const EVENT_DISPATCHED: &str = "scheduler.task.dispatched";
/// Ukończono krok atomowy.
pub const EVENT_STEP: &str = "scheduler.task.step";
/// Dostarczono wiadomość sterującą (`latency_steps` ≤ 1).
pub const EVENT_STEERED: &str = "scheduler.task.steered";
/// Zadanie oddało zasoby (pauza, wywłaszczenie, utrata warunku).
pub const EVENT_YIELDED: &str = "scheduler.task.yielded";
/// Wstrzymano.
pub const EVENT_PAUSED: &str = "scheduler.task.paused";
/// Wznowiono.
pub const EVENT_RESUMED: &str = "scheduler.task.resumed";
/// Zaplanowano ponowienie.
pub const EVENT_RETRY: &str = "scheduler.task.retry_scheduled";
/// Zakończono (z jawnym powodem).
pub const EVENT_FINISHED: &str = "scheduler.task.finished";
/// Wymuszono przerwanie wykonawczyni (brak punktu atomowego w czasie).
pub const EVENT_ABORTED: &str = "scheduler.task.aborted";
/// Podejrzenie pętli (powtarzane identyczne kroki).
pub const EVENT_LOOP: &str = "scheduler.task.loop_suspected";
/// Ostrzeżenie budżetu tła.
pub const EVENT_BUDGET_WARNING: &str = "scheduler.task.budget_warning";
/// Zadanie zakończyło się z niedostarczonym steeringiem (wiadomość przyszła w ostatnim kroku) —
/// Dyrygentka odpowiada na nią poza zadaniem.
pub const EVENT_STEER_UNCONSUMED: &str = "scheduler.task.steer_unconsumed";
/// Kill-switch zatrzymał zadania.
pub const EVENT_KILL_SWITCH: &str = "scheduler.kill_switch";
/// Odtworzono stan po restarcie.
pub const EVENT_RESTORED: &str = "scheduler.restored";

/// Wszystkie nazwy zdarzeń zadań (prefiks subskrypcji: `scheduler.`).
pub const ALL_EVENTS: [&str; 16] = [
    EVENT_SUBMITTED,
    EVENT_BLOCKED,
    EVENT_DISPATCHED,
    EVENT_STEP,
    EVENT_STEERED,
    EVENT_YIELDED,
    EVENT_PAUSED,
    EVENT_RESUMED,
    EVENT_RETRY,
    EVENT_FINISHED,
    EVENT_ABORTED,
    EVENT_LOOP,
    EVENT_BUDGET_WARNING,
    EVENT_STEER_UNCONSUMED,
    EVENT_KILL_SWITCH,
    EVENT_RESTORED,
];

/// Najdłuższy tytuł w zdarzeniu.
pub const EVENT_TITLE_CHARS: usize = 80;

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Poziom zdarzenia.
pub fn level_of(name: &str) -> Level {
    match name {
        EVENT_STEP | EVENT_BLOCKED => Level::Debug,
        EVENT_ABORTED
        | EVENT_LOOP
        | EVENT_BUDGET_WARNING
        | EVENT_KILL_SWITCH
        | EVENT_STEER_UNCONSUMED => Level::Warn,
        _ => Level::Info,
    }
}

/// Tytuł skrócony do [`EVENT_TITLE_CHARS`] znaków.
pub fn short_title(title: &str) -> String {
    let mut out: String = title.chars().take(EVENT_TITLE_CHARS).collect();
    if title.chars().count() > EVENT_TITLE_CHARS {
        out.push('…');
    }
    out
}

/// Zdarzenie zadania: `payload` dostaje pola `task` i `at_ms`.
pub fn task_event(
    name: &str,
    task: &TaskId,
    agent: Option<&str>,
    at_ms: u64,
    mut payload: Value,
) -> Event {
    if let Value::Object(map) = &mut payload {
        map.insert("task".into(), json!(task));
        map.insert("at_ms".into(), json!(at_ms));
    }
    let event = Event::new(event_kind(name), level_of(name), payload);
    match agent {
        Some(a) => event.with_agent(AgentId::new(a)),
        None => event,
    }
}

/// Zdarzenie bez zadania (kill-switch, odtworzenie).
pub fn global_event(name: &str, at_ms: u64, mut payload: Value) -> Event {
    if let Value::Object(map) = &mut payload {
        map.insert("at_ms".into(), json!(at_ms));
    }
    Event::new(event_kind(name), level_of(name), payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_payload() {
        assert!(ALL_EVENTS.iter().all(|n| n.starts_with("scheduler.")));
        let e = task_event(
            EVENT_FINISHED,
            &TaskId::new("a"),
            Some("delta"),
            7,
            json!({"result": "succeeded"}),
        );
        assert_eq!(e.kind.as_str(), EVENT_FINISHED);
        assert_eq!(e.payload["task"], "a");
        assert_eq!(e.payload["at_ms"], 7);
        assert_eq!(e.agent.map(|a| a.0), Some("delta".to_owned()));
        assert_eq!(level_of(EVENT_ABORTED), Level::Warn);
        assert_eq!(short_title(&"ą".repeat(100)).chars().count(), 81);
        assert_eq!(short_title("krótki"), "krótki");
        let g = global_event(EVENT_KILL_SWITCH, 3, json!({"cancelled": 2}));
        assert_eq!(g.payload["at_ms"], 3);
    }
}
