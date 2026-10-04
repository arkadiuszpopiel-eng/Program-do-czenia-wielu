//! F5-04 / ACC-F5-triggers-01: „most nie startuje z wyzwalacza” = 0/100 (zestaw:
//! `evals/F5/bridge-trigger-cases.json`). Łańcuch jak w aplikacji: `triggers-impl` → scheduler
//! (atrapa) → adapter wykonawczyni (pochodzenie zadania → `LaunchOrigin`) → `agent-backends`
//! (atrapa z prawdziwymi regułami `check_origin`), z jawną zgodą na harmonogram dla obu tras.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use agent_backends_contract as ab;
use agent_backends_contract::{
    AgentBackend, ApprovalDecision, ApprovalSink, BackendError, LaunchPolicy, PermissionRequest,
    ScheduleConsent,
};
use agent_backends_fake::FakeAgentBackend;
use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level, SessionId};
use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use scheduler_contract::{
    Assignee, BridgeKind, Dispatch, ExecutorKind, Scheduler, TaskClass, TaskError, TaskOrigin,
    TaskSpec, event_kind,
};
use scheduler_fake::FakeScheduler;
use triggers_contract::contract_tests::utc;
use triggers_contract::{
    Actor, CronExpr, RateLimit, TriggerAction, TriggerKind, TriggerSpec, Triggers,
};
use triggers_impl::{MemTriggerStore, NoFileWatch, TriggersModule};

struct Allow;

#[async_trait]
impl ApprovalSink for Allow {
    async fn request(&self, _request: PermissionRequest) -> Option<ApprovalDecision> {
        Some(ApprovalDecision::allow())
    }
}

#[derive(serde::Deserialize)]
struct Case {
    id: String,
    kind: String,
    attempt: String,
    bridge: String,
}

#[derive(serde::Deserialize)]
struct Set {
    threshold: Threshold,
    cases: Vec<Case>,
}

#[derive(serde::Deserialize)]
struct Threshold {
    bridge_starts: usize,
    of: usize,
}

struct Env {
    module: TriggersModule,
    scheduler: Arc<FakeScheduler>,
    backend: FakeAgentBackend,
    bus: FakeBus,
    seen: usize,
}

fn consent() -> LaunchPolicy {
    let mut policy = LaunchPolicy::default();
    for route in ["claude-code-cli", "codex-cli"] {
        policy
            .scheduled
            .insert(route.into(), ScheduleConsent { max_per_day: 100 });
    }
    policy
}

async fn settle() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

async fn make_env(policy: LaunchPolicy) -> Env {
    let start = utc("2026-06-01 10:00");
    let scheduler = Arc::new(FakeScheduler::starting_at(start));
    let bus = FakeBus::default();
    let mut module = TriggersModule::new(
        scheduler.clone(),
        Arc::new(MemTriggerStore::default()),
        Arc::new(NoFileWatch),
    )
    .unwrap()
    .with_start_ms(start);
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    settle().await;
    Env {
        module,
        scheduler,
        backend: FakeAgentBackend::new(Arc::new(Allow)).with_policy(policy),
        bus,
        seen: 0,
    }
}

/// Adapter wykonawczyni z `app-*`: zadanie schedulera → zadanie mostu z pochodzeniem zadania.
fn bridge_task(d: &Dispatch, bridge: BridgeKind) -> ab::TaskSpec {
    let mut t = ab::TaskSpec::user_request(bridge, "kontynuuj", "C:/repo", SessionId::new("s"));
    t.origin = d.spec.origin.launch_origin();
    t
}

#[derive(Default, Debug)]
struct Stats {
    spawn_rejected: usize,
    launch_refused: usize,
    violations: Vec<String>,
}

/// Każde nowe wysłanie: agentka próbuje mostu przez delegację i bezpośrednio.
async fn exercise(env: &mut Env, bridge: BridgeKind, launder: bool, stats: &mut Stats) {
    loop {
        let all = env.scheduler.dispatches();
        let Some(d) = all.get(env.seen).cloned() else {
            break;
        };
        env.seen += 1;
        let mut child = TaskSpec::new(
            d.task.child("most"),
            "deleguj do CLI",
            Assignee::AnyAgent,
            TaskClass::User,
            TaskOrigin::User,
        );
        child.executor = ExecutorKind::Bridge(bridge);
        match env.scheduler.spawn(d.dispatch, vec![child]) {
            Err(TaskError::BridgeNotAllowed { .. }) => stats.spawn_rejected += 1,
            other => stats
                .violations
                .push(format!("{}: podzadanie-most {other:?}", d.task)),
        }
        match env.backend.submit_task(bridge_task(&d, bridge)).await {
            Err(BackendError::LaunchRefused { .. }) => stats.launch_refused += 1,
            other => stats.violations.push(format!("{}: most {other:?}", d.task)),
        }
        if launder && d.spec.parent.is_none() {
            let child = TaskSpec::new(
                d.task.child("jako-uzytkownik"),
                "zrób to jako użytkownik",
                Assignee::AnyAgent,
                TaskClass::User,
                TaskOrigin::User,
            );
            env.scheduler.spawn(d.dispatch, vec![child]).unwrap();
        }
    }
}

fn spec_for(case: &Case, now: u64) -> TriggerSpec {
    let kind = match case.kind.as_str() {
        "cron" => TriggerKind::Cron {
            expr: CronExpr::parse("* * * * *").unwrap(),
        },
        "interval" => TriggerKind::Interval {
            every_ms: 60_000,
            start_ms: None,
        },
        "once" => TriggerKind::Once {
            at_ms: now + 30_000,
        },
        "file_in_dir" => TriggerKind::FileInDir {
            dir: "C:\\Users\\Ja\\Pobrane".into(),
            pattern: Some("*.pdf".into()),
        },
        "new_message" => TriggerKind::NewMessage { session: None },
        "task_finished" => TriggerKind::TaskFinished {
            task_prefix: None,
            outcome: triggers_contract::FinishFilter::Any,
        },
        _ => TriggerKind::Manual,
    };
    let action = TriggerAction::new("zadanie z wyzwalacza", "przejrzyj kod i popraw błędy");
    TriggerSpec::new(
        case.id.as_str(),
        "przypadek F5-04",
        Actor::User,
        kind,
        action,
    )
}

async fn publish(bus: &FakeBus, name: &str, payload: serde_json::Value) {
    bus.publish(Event::new(event_kind(name), Level::Info, payload))
        .await
        .unwrap();
    settle().await;
}

async fn fire(env: &Env, case: &Case) {
    match case.kind.as_str() {
        "cron" | "interval" => tokio::time::sleep(Duration::from_millis(61_000)).await,
        "once" => tokio::time::sleep(Duration::from_millis(31_000)).await,
        "file_in_dir" => drop(
            env.module
                .file_created("C:\\Users\\Ja\\Pobrane\\faktura.pdf"),
        ),
        "new_message" => {
            let p = serde_json::json!({"session": "s1", "turn": "t1", "role": "user"});
            publish(&env.bus, "session.turn.appended", p).await;
        }
        "task_finished" => {
            let p = serde_json::json!({"task": "raport/1", "result": "succeeded", "origin": {"origin": "user"}});
            publish(&env.bus, "scheduler.task.finished", p).await;
        }
        _ => drop(env.module.fire_now(&case.id.as_str().into(), Actor::User)),
    }
    settle().await;
}

#[tokio::test(start_paused = true)]
async fn bridge_never_starts_from_trigger_0_of_100() {
    let set: Set =
        serde_json::from_str(include_str!("../../../evals/F5/bridge-trigger-cases.json")).unwrap();
    assert_eq!(set.cases.len(), set.threshold.of);
    let mut stats = Stats::default();
    let mut starts = 0usize;
    let mut by_attempt: BTreeMap<String, usize> = BTreeMap::new();
    for case in &set.cases {
        let mut env = make_env(consent()).await;
        let bridge = if case.bridge == "codex" {
            BridgeKind::Codex
        } else {
            BridgeKind::ClaudeCode
        };
        let now = env.module.now_ms().unwrap();
        let mut spec = spec_for(case, now);
        let created = match case.attempt.as_str() {
            "direct_executor" => {
                spec.action.executor = ExecutorKind::Bridge(bridge);
                env.module.create(spec, Actor::User)
            }
            "allow_bridges_agent" => {
                let delta = Actor::Agent("delta".into());
                spec.owner = delta.clone();
                spec.allow_bridges = true;
                spec.action.executor = ExecutorKind::Bridge(bridge);
                spec.rate = RateLimit {
                    max_fires: 1,
                    per_ms: 86_400_000,
                };
                env.module.create(spec, delta)
            }
            "allow_bridges_event" => {
                spec.allow_bridges = true;
                spec.action.executor = ExecutorKind::Bridge(bridge);
                spec.rate = RateLimit {
                    max_fires: 1,
                    per_ms: 86_400_000,
                };
                env.module.create(spec, Actor::User)
            }
            _ => env.module.create(spec, Actor::User),
        };
        let expect_rejected = !matches!(case.attempt.as_str(), "delegation" | "launder_origin");
        if expect_rejected == created.is_ok() {
            stats
                .violations
                .push(format!("{}: utworzenie {created:?}", case.id));
        }
        fire(&env, case).await;
        exercise(
            &mut env,
            bridge,
            case.attempt == "launder_origin",
            &mut stats,
        )
        .await;
        let started = env.backend.submitted().len();
        starts += started;
        *by_attempt.entry(case.attempt.clone()).or_default() += 1;
        if !expect_rejected && env.seen == 0 {
            stats
                .violations
                .push(format!("{}: wyzwalacz nie dał zadania", case.id));
        }
    }
    eprintln!(
        "F5-04: most uruchomiony z wyzwalacza {starts}/{} (odrzucone podzadania-mosty: {}, odmowy mostu: {}, przypadki: {by_attempt:?})",
        set.cases.len(),
        stats.spawn_rejected,
        stats.launch_refused
    );
    assert!(stats.violations.is_empty(), "{:#?}", stats.violations);
    assert_eq!(starts, set.threshold.bridge_starts);
    assert!(stats.launch_refused >= 50, "zestaw nie może być pusty");
}

/// Kontrole (poza zestawem): żądanie użytkownika — most startuje. Regresja CX-d (AGENTS.md:
/// mostów CLI nie uruchamia się z harmonogramu): harmonogram czasowy użytkownika z `allow_bridges`
/// i niezerową zgodą dzienną trasy **nie** startuje mostu — odrzucony przy tworzeniu, a zadanie
/// z pochodzeniem `Schedule` nie przechodzi ani przez scheduler, ani przez most.
#[tokio::test(start_paused = true)]
async fn controls_user_request_starts_schedule_never() {
    let mut env = make_env(consent()).await;
    let mut user = TaskSpec::new(
        "prosba",
        "prośba",
        Assignee::AnyAgent,
        TaskClass::User,
        TaskOrigin::User,
    );
    user.executor = ExecutorKind::Bridge(BridgeKind::ClaudeCode);
    env.scheduler.submit(vec![user]).unwrap();
    let d = env.scheduler.dispatches().pop().unwrap();
    env.seen = env.scheduler.dispatches().len();
    env.backend
        .submit_task(bridge_task(&d, BridgeKind::ClaudeCode))
        .await
        .unwrap();
    assert_eq!(env.backend.submitted().len(), 1);

    let mut schedule = TriggerSpec::new(
        "nocny-przeglad",
        "nocny przegląd",
        Actor::User,
        TriggerKind::Cron {
            expr: CronExpr::parse("0 3 * * *").unwrap(),
        },
        TriggerAction::new("przegląd", "przejrzyj zmiany"),
    );
    schedule.allow_bridges = true;
    schedule.action.executor = ExecutorKind::Bridge(BridgeKind::Codex);
    schedule.rate = RateLimit {
        max_fires: 1,
        per_ms: 86_400_000,
    };
    let created = env.module.create(schedule, Actor::User);
    assert!(created.is_err(), "harmonogram z mostem: {created:?}");
    assert!(
        env.module
            .fire_now(&"nocny-przeglad".into(), Actor::User)
            .is_err()
    );
    settle().await;
    assert_eq!(env.scheduler.dispatches().len(), env.seen, "brak zadania");

    let origin = TaskOrigin::Schedule {
        schedule_id: "nocny-przeglad".into(),
    };
    let mut scheduled = TaskSpec::new(
        "z-harmonogramu",
        "z harmonogramu",
        Assignee::AnyAgent,
        TaskClass::Agent,
        origin.clone(),
    );
    scheduled.executor = ExecutorKind::Bridge(BridgeKind::Codex);
    assert!(matches!(
        env.scheduler.submit(vec![scheduled]),
        Err(TaskError::BridgeNotAllowed { .. })
    ));
    let mut task = bridge_task(&d, BridgeKind::Codex);
    task.origin = origin.launch_origin();
    let err = env.backend.submit_task(task).await.unwrap_err();
    assert!(matches!(err, BackendError::LaunchRefused { .. }), "{err:?}");
    assert_eq!(
        env.backend.submitted().len(),
        1,
        "tylko żądanie użytkownika"
    );
}
