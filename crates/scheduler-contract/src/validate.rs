//! Limity i walidacja specyfikacji (niezależna od stanu schedulera). Każda wartość jest
//! skończona — to podstawa gwarancji „każde zadanie kończy się w skończonym czasie”.

use std::collections::BTreeSet;

use scheduler_lite_contract::Resource;

use crate::error::TaskError;
use crate::spec::{Assignee, ExecutorKind, TaskSpec};

/// Najwięcej zadań w jednym zgłoszeniu.
pub const MAX_TASKS_PER_SUBMIT: usize = 256;
/// Najwięcej aktywnych (niezakończonych) zadań.
pub const MAX_ACTIVE_TASKS: usize = 4096;
/// Najwięcej zależności zadania.
pub const MAX_DEPS: usize = 64;
/// Najwięcej zasobów zadania.
pub const MAX_RESOURCES: usize = 16;
/// Najdłuższy tytuł (znaki).
pub const MAX_TITLE_CHARS: usize = 200;
/// Największy ładunek (bajty JSON).
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
/// Największy wynik (bajty JSON).
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
/// Najwięcej kroków atomowych.
pub const MAX_STEPS: u32 = 10_000;
/// Najdłuższy łączny czas wykonania (24 h).
pub const MAX_WALL_MS: u64 = 24 * 3600 * 1000;
/// Najwięcej prób.
pub const MAX_ATTEMPTS: u32 = 10;
/// Najdłuższy odstęp ponowienia (1 h).
pub const MAX_BACKOFF_MS: u64 = 3600 * 1000;
/// Domyślny termin, gdy zgłaszająca nie poda (24 h od zgłoszenia).
pub const DEFAULT_DEADLINE_MS: u64 = 24 * 3600 * 1000;
/// Najdalszy termin (30 dni od zgłoszenia).
pub const MAX_DEADLINE_AHEAD_MS: u64 = 30 * 24 * 3600 * 1000;
/// Najwięcej oczekujących wiadomości sterujących na zadanie.
pub const MAX_PENDING_STEERS: usize = 64;
/// Czas na dojście do punktu atomowego po żądaniu zatrzymania (anulowanie ≤ 2 s, PLAN §9.1);
/// potem wykonawczyni jest przerywana siłą.
pub const STOP_GRACE_MS: u64 = 2_000;
/// Ile identycznych kroków z rzędu to podejrzenie pętli (zdarzenie dla Marszałka).
pub const LOOP_REPEATS: u32 = 3;
/// Jak długo trzymać zakończone zadania (Oś czasu), zanim zostaną usunięte z pamięci.
pub const RETENTION_MS: u64 = 24 * 3600 * 1000;

fn invalid(spec: &TaskSpec, reason: impl Into<String>) -> TaskError {
    TaskError::InvalidSpec {
        task: spec.id.clone(),
        reason: reason.into(),
    }
}

/// Walidacja pojedynczej specyfikacji w chwili `now_ms`.
pub fn validate_spec(spec: &TaskSpec, now_ms: u64) -> Result<(), TaskError> {
    if !spec.id.is_valid() {
        return Err(invalid(spec, "niepoprawny identyfikator"));
    }
    let title = spec.title.trim();
    if title.is_empty() || spec.title.chars().count() > MAX_TITLE_CHARS {
        return Err(invalid(spec, "tytuł pusty albo za długi"));
    }
    if spec.deps.len() > MAX_DEPS {
        return Err(invalid(spec, "za dużo zależności"));
    }
    if spec.deps.iter().any(|d| d.task == spec.id) || spec.parent.as_ref() == Some(&spec.id) {
        return Err(TaskError::Cycle {
            path: vec![spec.id.clone(), spec.id.clone()],
        });
    }
    validate_resources(spec)?;
    validate_limits(spec, now_ms)?;
    if let ExecutorKind::Bridge(_) = spec.executor
        && !spec.origin.may_target_bridge()
    {
        return Err(TaskError::BridgeNotAllowed {
            task: spec.id.clone(),
            origin: format!("{:?}", spec.origin.launch_origin()),
        });
    }
    let payload = serde_json::to_vec(&spec.payload).map_or(usize::MAX, |b| b.len());
    if payload > MAX_PAYLOAD_BYTES {
        return Err(invalid(spec, "ładunek za duży"));
    }
    Ok(())
}

fn validate_resources(spec: &TaskSpec) -> Result<(), TaskError> {
    if spec.resources.len() > MAX_RESOURCES {
        return Err(invalid(spec, "za dużo zasobów"));
    }
    let unique: BTreeSet<&Resource> = spec.resources.iter().collect();
    if unique.len() != spec.resources.len() {
        return Err(invalid(spec, "powtórzony zasób"));
    }
    if matches!(spec.assignee, Assignee::System(_)) && spec.resources.contains(&Resource::Speaker) {
        return Err(invalid(spec, "usługa systemowa nie może trzymać głośnika"));
    }
    if spec
        .resources
        .iter()
        .any(|r| matches!(r, Resource::File(p) if p.trim().is_empty()))
    {
        return Err(invalid(spec, "pusta ścieżka zasobu"));
    }
    Ok(())
}

fn validate_limits(spec: &TaskSpec, now_ms: u64) -> Result<(), TaskError> {
    let b = &spec.budget;
    if b.max_steps == 0 || b.max_steps > MAX_STEPS {
        return Err(invalid(spec, "budżet kroków poza zakresem"));
    }
    if b.max_wall_ms == 0 || b.max_wall_ms > MAX_WALL_MS {
        return Err(invalid(spec, "budżet czasu poza zakresem"));
    }
    if b.max_cost_micro_pln == Some(0) {
        return Err(invalid(spec, "zerowy budżet kosztu"));
    }
    let r = &spec.retry;
    if r.max_attempts == 0 || r.max_attempts > MAX_ATTEMPTS {
        return Err(invalid(spec, "liczba prób poza zakresem"));
    }
    if r.multiplier == 0
        || r.max_backoff_ms > MAX_BACKOFF_MS
        || r.initial_backoff_ms > r.max_backoff_ms
    {
        return Err(invalid(spec, "niepoprawne odstępy ponowień"));
    }
    let w = &spec.window;
    if let Some(deadline) = w.deadline_ms {
        if deadline <= now_ms {
            return Err(invalid(spec, "termin już minął"));
        }
        if deadline - now_ms > MAX_DEADLINE_AHEAD_MS {
            return Err(invalid(spec, "termin za daleko"));
        }
    }
    if let (Some(nb), Some(deadline)) = (w.not_before_ms, w.deadline_ms)
        && nb >= deadline
    {
        return Err(invalid(spec, "okno „nie wcześniej niż” po terminie"));
    }
    if w.not_before_ms
        .is_some_and(|nb| nb.saturating_sub(now_ms) > MAX_DEADLINE_AHEAD_MS)
    {
        return Err(invalid(spec, "start za daleko"));
    }
    Ok(())
}

/// Efektywny termin: podany albo domyślny (zgłoszenie + 24 h, lecz nie przed oknem startu).
pub fn effective_deadline(spec: &TaskSpec, now_ms: u64) -> u64 {
    spec.window.deadline_ms.unwrap_or_else(|| {
        let start = spec.window.not_before_ms.unwrap_or(now_ms).max(now_ms);
        start.saturating_add(DEFAULT_DEADLINE_MS)
    })
}

/// Rozmiar wyniku w bajtach JSON.
pub fn output_size(output: &crate::state::TaskOutput) -> usize {
    serde_json::to_vec(output).map_or(usize::MAX, |b| b.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Dependency, TaskClass, TaskOrigin};
    use agent_backends_contract::BridgeKind;

    fn spec() -> TaskSpec {
        TaskSpec::new(
            "t1",
            "Zadanie",
            Assignee::AnyAgent,
            TaskClass::Agent,
            TaskOrigin::User,
        )
    }

    #[test]
    fn accepts_defaults_and_rejects_bad_values() {
        assert!(validate_spec(&spec(), 0).is_ok());
        let mut s = spec();
        s.title = " ".into();
        assert!(validate_spec(&s, 0).is_err());
        let mut s = spec();
        s.deps.push(Dependency::on("t1"));
        assert!(matches!(validate_spec(&s, 0), Err(TaskError::Cycle { .. })));
        let mut s = spec();
        s.resources = vec![Resource::Mic, Resource::Mic];
        assert!(validate_spec(&s, 0).is_err());
        let mut s = spec();
        s.assignee = Assignee::System("pamięć".into());
        s.resources = vec![Resource::Speaker];
        assert!(validate_spec(&s, 0).is_err());
        let mut s = spec();
        s.budget.max_wall_ms = MAX_WALL_MS + 1;
        assert!(validate_spec(&s, 0).is_err());
        let mut s = spec();
        s.retry.max_attempts = 0;
        assert!(validate_spec(&s, 0).is_err());
        let mut s = spec();
        s.window.deadline_ms = Some(10);
        assert!(validate_spec(&s, 10).is_err());
        s.window.not_before_ms = Some(20);
        assert!(validate_spec(&s, 0).is_err());
        let mut s = spec();
        s.payload = serde_json::json!("x".repeat(MAX_PAYLOAD_BYTES));
        assert!(validate_spec(&s, 0).is_err());
    }

    /// Regresja CX-d: harmonogram nie celuje w most (AGENTS.md) — tylko `User`.
    #[test]
    fn bridges_only_from_user() {
        let mut s = spec();
        s.executor = ExecutorKind::Bridge(BridgeKind::ClaudeCode);
        assert!(validate_spec(&s, 0).is_ok());
        for origin in [
            TaskOrigin::Schedule {
                schedule_id: "s".into(),
            },
            TaskOrigin::Trigger {
                trigger_id: "t".into(),
                depth: 1,
            },
            TaskOrigin::Improver,
            TaskOrigin::Agent {
                persona: "delta".into(),
            },
            TaskOrigin::System {
                service: "x".into(),
            },
        ] {
            s.origin = origin;
            assert!(matches!(
                validate_spec(&s, 0),
                Err(TaskError::BridgeNotAllowed { .. })
            ));
        }
    }

    #[test]
    fn default_deadline_is_finite() {
        let mut s = spec();
        assert_eq!(effective_deadline(&s, 5), 5 + DEFAULT_DEADLINE_MS);
        s.window.not_before_ms = Some(100);
        assert_eq!(effective_deadline(&s, 5), 100 + DEFAULT_DEADLINE_MS);
        s.window.deadline_ms = Some(50);
        assert_eq!(effective_deadline(&s, 5), 50);
    }
}
