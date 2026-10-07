//! Współdzielone testy kontraktowe `AgentBackend` (feature `contract-tests`): uruchamiane na
//! `FakeAgentBackend` i na prawdziwych mostach (`-impl`) z fałszywym CLI.

pub mod scenario;

use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use core_bus_contract::SessionId;
use futures_util::StreamExt;

use crate::approval::{ApprovalDecision, ApprovalSink, PermissionRequest};
use crate::backend::AgentBackend;
use crate::error::{BackendError, LaunchRefusal};
use crate::event::{AgentEvent, AgentEventEnvelope};
use crate::task::{BridgeKind, LaunchOrigin, TaskId, TaskSpec};
use scenario::{Scenario, permission_result};

/// Budżet czasowy testu (crates/README.md „Testy budżetów czasowych”): ściśle przy
/// `ALFA_PERF_BUDGETS=1`, inaczej ×10 (łapie tylko patologiczne regresje). Zawsze wypisuje budżet.
pub fn budget(strict: Duration, what: &str) -> Duration {
    let strict_mode = std::env::var("ALFA_PERF_BUDGETS").is_ok_and(|v| v == "1");
    let limit = if strict_mode { strict } else { strict * 10 };
    eprintln!("budżet `{what}`: {limit:?} (ściśle: {strict_mode})");
    limit
}

/// Kanał zatwierdzeń do testów: zapisuje prośby; decyzje z listy (cyklicznie) albo `None`.
#[derive(Debug, Default)]
pub struct RecordingSink {
    pattern: Vec<bool>,
    seen: Mutex<Vec<PermissionRequest>>,
}

impl RecordingSink {
    /// Decyzje cyklicznie wg wzorca (`true` = zgoda); pusty wzorzec = zawsze `None`.
    pub fn with_pattern(pattern: Vec<bool>) -> Arc<Self> {
        Arc::new(Self {
            pattern,
            seen: Mutex::new(Vec::new()),
        })
    }

    /// Prośby, które dotarły.
    pub fn seen(&self) -> Vec<PermissionRequest> {
        self.seen.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

#[async_trait]
impl ApprovalSink for RecordingSink {
    async fn request(&self, request: PermissionRequest) -> Option<ApprovalDecision> {
        let mut seen = self.seen.lock().unwrap_or_else(|p| p.into_inner());
        let n = seen.len();
        seen.push(request);
        if self.pattern.is_empty() {
            return None;
        }
        Some(if self.pattern[n % self.pattern.len()] {
            ApprovalDecision::allow()
        } else {
            ApprovalDecision::deny("odmowa testowa")
        })
    }
}

/// Środowisko zestawu: most i katalog źródłowy (repozytorium testowe).
#[derive(Debug, Clone)]
pub struct Env {
    /// Most.
    pub bridge: BridgeKind,
    /// Katalog źródłowy.
    pub source: PathBuf,
}

impl Env {
    /// Zadanie użytkownika ze scenariuszem.
    pub fn spec(&self, scenario: &Scenario) -> TaskSpec {
        TaskSpec::user_request(
            self.bridge,
            scenario.prompt(),
            self.source.clone(),
            SessionId::new("contract"),
        )
    }
}

/// Zbiera wszystkie zdarzenia zadania (do zdarzenia końcowego) z limitem czasu.
pub async fn collect<B: AgentBackend + ?Sized>(
    backend: &B,
    task: &TaskId,
    limit: Duration,
) -> Vec<AgentEventEnvelope> {
    let mut stream = backend.events(task).unwrap_or_else(|e| panic!("{e}"));
    let mut out = Vec::new();
    let deadline = tokio::time::Instant::now() + limit;
    while let Ok(Some(ev)) = tokio::time::timeout_at(deadline, stream.next()).await {
        let terminal = ev.event.is_terminal();
        out.push(ev);
        if terminal {
            break;
        }
    }
    out
}

/// Gramatyka: `Started` pierwsze, dokładnie jedno zdarzenie końcowe na końcu, `seq` bez luk,
/// wszystkie `unverified_by_alfa`.
pub fn assert_grammar(events: &[AgentEventEnvelope]) {
    assert!(
        matches!(
            events.first().map(|e| &e.event),
            Some(AgentEvent::Started { .. })
        ),
        "pierwsze zdarzenie musi być Started: {events:?}"
    );
    let terminals = events.iter().filter(|e| e.event.is_terminal()).count();
    assert_eq!(
        terminals, 1,
        "dokładnie jedno zdarzenie końcowe: {events:?}"
    );
    assert!(events.last().is_some_and(|e| e.event.is_terminal()));
    for (i, e) in events.iter().enumerate() {
        assert_eq!(e.seq, i as u64, "luka w seq");
        assert!(
            e.unverified_by_alfa,
            "zdarzenie bez oznaczenia unverified_by_alfa"
        );
    }
}

fn terminal(events: &[AgentEventEnvelope]) -> &AgentEvent {
    events
        .last()
        .map(|e| &e.event)
        .unwrap_or_else(|| panic!("brak zdarzeń"))
}

/// Pełny przebieg kończy się `Done` bez błędu.
pub async fn ok_flow<B: AgentBackend>(backend: &B, env: &Env) {
    let h = backend
        .submit_task(env.spec(&Scenario::Ok))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_ne!(
        h.workdir, env.source,
        "most nie może pracować w katalogu użytkownika"
    );
    let events = collect(backend, &h.task, Duration::from_secs(60)).await;
    assert_grammar(&events);
    assert!(
        matches!(terminal(&events), AgentEvent::Done { result } if !result.is_error),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e.event, AgentEvent::Output { .. }))
    );
}

/// 20/20 próśb trafia do `ApprovalSink`, decyzje wracają do CLI w tej samej kolejności.
pub async fn permissions_reach_sink<B: AgentBackend>(backend: &B, sink: &RecordingSink, env: &Env) {
    let h = backend
        .submit_task(env.spec(&Scenario::Permission(20)))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let events = collect(backend, &h.task, Duration::from_secs(120)).await;
    assert_grammar(&events);
    let requested = events
        .iter()
        .filter(|e| matches!(e.event, AgentEvent::PermissionRequest { .. }))
        .count();
    let resolved = events
        .iter()
        .filter(|e| {
            matches!(
                e.event,
                AgentEvent::PermissionResolved {
                    timed_out: false,
                    ..
                }
            )
        })
        .count();
    let seen = sink.seen().into_iter().filter(|r| r.task == h.task).count();
    assert_eq!((requested, resolved, seen), (20, 20, 20), "{events:?}");
    let expected: Vec<bool> = (0..20).map(|i| i % 2 == 0).collect();
    match terminal(&events) {
        AgentEvent::Done { result } => assert_eq!(result.text, permission_result(&expected)),
        other => panic!("oczekiwano Done, jest {other:?}"),
    }
}

/// Decyzja przez `AgentBackend::approve` (kanał zwraca `None`).
pub async fn approve_through_backend<B: AgentBackend>(backend: &B, env: &Env) {
    let h = backend
        .submit_task(env.spec(&Scenario::Permission(3)))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut stream = backend.events(&h.task).unwrap_or_else(|e| panic!("{e}"));
    let mut last = None;
    let limit = Duration::from_secs(60);
    while let Ok(Some(ev)) = tokio::time::timeout(limit, stream.next()).await {
        if let AgentEvent::PermissionRequest { request } = &ev.event {
            let decision = if request.tool.is_empty() {
                ApprovalDecision::deny("x")
            } else {
                ApprovalDecision::allow()
            };
            backend
                .approve(&request.id, decision.clone())
                .await
                .unwrap_or_else(|e| panic!("{e}"));
            let again = backend.approve(&request.id, decision).await;
            assert!(matches!(
                again,
                Err(BackendError::UnknownPermissionRequest(_))
            ));
        }
        let end = ev.event.is_terminal();
        last = Some(ev.event);
        if end {
            break;
        }
    }
    match last {
        Some(AgentEvent::Done { result }) => {
            assert_eq!(result.text, permission_result(&[true, true, true]));
        }
        other => panic!("oczekiwano Done, jest {other:?}"),
    }
}

/// Anulowanie wiszącego zadania kończy strumień `Error(Cancelled)` w budżecie 2 s.
pub async fn cancel_within_budget<B: AgentBackend>(backend: &B, env: &Env) {
    let h = backend
        .submit_task(env.spec(&Scenario::Hang))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut stream = backend.events(&h.task).unwrap_or_else(|e| panic!("{e}"));
    // Czekamy, aż CLI faktycznie ruszy (pierwsze wyjście), żeby anulować działający proces.
    while let Ok(Some(ev)) = tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
        if matches!(ev.event, AgentEvent::Output { .. }) {
            break;
        }
    }
    let limit = budget(Duration::from_secs(2), "anulowanie zadania mostu");
    let started = Instant::now();
    backend
        .cancel(&h.task)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let events = collect(backend, &h.task, limit).await;
    let took = started.elapsed();
    eprintln!("anulowanie: {took:?}");
    assert!(took <= limit, "anulowanie trwało {took:?}");
    assert_grammar(&events);
    assert!(matches!(
        terminal(&events),
        AgentEvent::Error {
            error: BackendError::Cancelled
        }
    ));
    assert!(matches!(
        backend.cancel(&h.task).await,
        Ok(()) | Err(BackendError::TaskFinished)
    ));
}

/// Wyzwalacz, Ulepszacz i harmonogram bez zgody → odmowa bez zadania.
pub async fn origins_refused<B: AgentBackend>(backend: &B, env: &Env) {
    let cases = [
        (
            LaunchOrigin::Trigger {
                trigger_id: "t".into(),
            },
            LaunchRefusal::Trigger,
        ),
        (LaunchOrigin::Improver, LaunchRefusal::Improver),
        (
            LaunchOrigin::Scheduled {
                schedule_id: "s".into(),
            },
            LaunchRefusal::ScheduleWithoutConsent,
        ),
    ];
    for (origin, refusal) in cases {
        let mut spec = env.spec(&Scenario::Ok);
        spec.origin = origin;
        assert_eq!(
            backend.submit_task(spec).await,
            Err(BackendError::LaunchRefused { refusal })
        );
    }
}

/// Nieznane identyfikatory.
pub async fn unknown_ids<B: AgentBackend>(backend: &B) {
    let t = TaskId("brak".into());
    assert!(matches!(
        backend.events(&t),
        Err(BackendError::UnknownTask(_))
    ));
    assert!(matches!(
        backend.cancel(&t).await,
        Err(BackendError::UnknownTask(_))
    ));
    assert!(matches!(
        backend.steer(&t, "x".into()).await,
        Err(BackendError::UnknownTask(_))
    ));
    let r = crate::approval::PermissionRequestId("brak".into());
    assert!(matches!(
        backend.approve(&r, ApprovalDecision::allow()).await,
        Err(BackendError::UnknownPermissionRequest(_))
    ));
}

/// Crash CLI w połowie → `Error(CliExited)`.
pub async fn crash_is_error<B: AgentBackend>(backend: &B, env: &Env) {
    let h = backend
        .submit_task(env.spec(&Scenario::Crash))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let events = collect(backend, &h.task, Duration::from_secs(60)).await;
    assert_grammar(&events);
    assert!(
        matches!(
            terminal(&events),
            AgentEvent::Error {
                error: BackendError::CliExited { .. }
            }
        ),
        "{events:?}"
    );
}

/// Steering trafia do działającego zadania.
pub async fn steering_reaches_task<B: AgentBackend>(backend: &B, env: &Env) {
    let h = backend
        .submit_task(env.spec(&Scenario::Steer))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut stream = backend.events(&h.task).unwrap_or_else(|e| panic!("{e}"));
    while let Ok(Some(ev)) = tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
        if matches!(ev.event, AgentEvent::Output { .. }) {
            break;
        }
    }
    backend
        .steer(&h.task, "zmień plan".into())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let events = collect(backend, &h.task, Duration::from_secs(60)).await;
    let echoed = events.iter().any(|e| {
        matches!(&e.event, AgentEvent::Output { text, .. } if text.contains("steer:zmień plan"))
    });
    assert!(echoed, "{events:?}");
    assert!(
        matches!(terminal(&events), AgentEvent::Done { .. }),
        "{events:?}"
    );
}

/// Uruchamia cały zestaw. `factory(sink)` daje świeży backend z podanym kanałem zatwierdzeń.
pub async fn run_all<B, F, Fut>(env: Env, factory: F)
where
    B: AgentBackend,
    F: Fn(Arc<dyn ApprovalSink>) -> Fut,
    Fut: Future<Output = B>,
{
    let none = RecordingSink::with_pattern(Vec::new());
    ok_flow(&factory(none.clone()).await, &env).await;
    let alternating = RecordingSink::with_pattern(vec![true, false]);
    permissions_reach_sink(&factory(alternating.clone()).await, &alternating, &env).await;
    approve_through_backend(&factory(none.clone()).await, &env).await;
    cancel_within_budget(&factory(none.clone()).await, &env).await;
    origins_refused(&factory(none.clone()).await, &env).await;
    unknown_ids(&factory(none.clone()).await).await;
    crash_is_error(&factory(none.clone()).await, &env).await;
    steering_reaches_task(&factory(none).await, &env).await;
}
