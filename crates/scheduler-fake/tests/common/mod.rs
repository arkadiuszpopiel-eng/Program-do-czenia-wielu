//! Sterownik scenariuszy na atrapie: zgłoszenia, zdarzenia zewnętrzne (steering, anulowanie,
//! pauzy, mowa przez `scheduler-lite`, warunki, obsada, budżet, kill-switch, restart) i sprawdzanie
//! niezmienników po każdym kroku czasu.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod scenario_gen;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use cost_meter_contract::{BudgetDecision, BudgetNotice, BudgetScope};
use scheduler_contract::{
    AgentSlot, EVENT_DISPATCHED, EVENT_FINISHED, EVENT_STEERED, Holder, Lease, LeaseRequest,
    Priority, Resource, Roster, SchedError, Scheduler, SchedulerLite, Steer, SystemConditions,
    TaskId, TaskState,
};
use scheduler_fake::FakeScheduler;

use scenario_gen::{Ext, Scenario};

type LeaseFut = Pin<Box<dyn Future<Output = Result<Lease, SchedError>> + Send>>;

struct Voice {
    fut: Option<LeaseFut>,
    lease: Option<Lease>,
    counted: bool,
    hold_ms: u64,
    release_at: u64,
}

/// Wynik przebiegu scenariusza.
#[derive(Debug, Default)]
pub struct Report {
    pub violations: Vec<String>,
    pub tasks: usize,
    pub steers_delivered: usize,
    pub max_steer_latency: u64,
    pub voice_granted: usize,
    pub max_parallel_agents: usize,
}

fn poll<F: Future + Unpin>(fut: &mut F) -> Option<F::Output> {
    let mut cx = Context::from_waker(Waker::noop());
    match Pin::new(fut).poll(&mut cx) {
        Poll::Ready(v) => Some(v),
        Poll::Pending => None,
    }
}

fn small_roster() -> Roster {
    let mut r = Roster::default();
    r.agents
        .retain(|a| a.persona.as_str() == "alfa" || a.persona.as_str() == "delta");
    r.agents.push(AgentSlot {
        persona: "beta".into(),
        roles: vec![],
        available: false,
        max_parallel: 1,
    });
    r.max_parallel_total = 2;
    r
}

fn block() -> BudgetDecision {
    BudgetDecision::Block {
        notice: BudgetNotice {
            scope: BudgetScope::Background,
            spent_micro_pln: 1,
            estimate_micro_pln: 1,
            limit_micro_pln: 1,
            pct_after: 200,
        },
    }
}

/// Niezmienniki chwilowe: wyłączność (zadania + mowa), zasoby tylko u zadań w toku.
fn check_now(f: &FakeScheduler, voices: &[Voice], report: &mut Report) {
    let mut owners: BTreeMap<Resource, String> = BTreeMap::new();
    let mut claim = |r: &Resource, who: String, report: &mut Report| {
        if let Some(prev) = owners.insert(r.clone(), who.clone()) {
            report
                .violations
                .push(format!("t={} {r} naraz: {prev} i {who}", f.now_ms()));
        }
    };
    let agents = f
        .tasks()
        .iter()
        .filter(|v| matches!(v.state, TaskState::Running { agent: Some(_), .. }))
        .count();
    report.max_parallel_agents = report.max_parallel_agents.max(agents);
    for (task, rs) in f.held() {
        match f.task(&task).map(|v| v.state) {
            Some(TaskState::Running { .. }) => {}
            other => report
                .violations
                .push(format!("{task} trzyma zasoby w stanie {other:?}")),
        }
        for r in rs {
            claim(&r, task.to_string(), report);
        }
    }
    for v in voices {
        if let Some(l) = v.lease.as_ref().filter(|l| !l.is_revoked()) {
            claim(l.resource(), format!("mowa {}", l.id().0), report);
        }
    }
}

fn apply(f: &FakeScheduler, sc: &Scenario, ext: &Ext, voices: &mut Vec<Voice>) {
    let id = |i: usize| TaskId::new(format!("t{i}"));
    match ext {
        Ext::Steer(i, voice) => {
            let steer = if *voice {
                Steer::voice("korekta głosem")
            } else {
                Steer::text("korekta")
            };
            let _ = f.steer(&id(*i), steer);
        }
        Ext::Cancel(i) => drop(f.cancel(&id(*i), "scenariusz")),
        Ext::Pause(i) => drop(f.pause(&id(*i))),
        Ext::Resume(i) => drop(f.resume(&id(*i))),
        Ext::Voice {
            resource,
            hold_ms,
            max_wait_ms,
        } => {
            let req = LeaseRequest::new(
                resource.clone(),
                Holder::User,
                Priority::UserSpeech,
                Duration::from_millis(*max_wait_ms),
            );
            let locks = std::sync::Arc::clone(f.core().locks());
            let mut fut: LeaseFut = Box::pin(async move { locks.acquire(req).await });
            // Pierwsze odpytanie rejestruje żądanie w tablicy blokad (przyszłości są leniwe).
            let mut voice = Voice {
                fut: None,
                lease: None,
                counted: false,
                hold_ms: *hold_ms,
                release_at: u64::MAX,
            };
            match poll(&mut fut) {
                None => voice.fut = Some(fut),
                Some(Ok(lease)) => {
                    voice.release_at = f.now_ms() + hold_ms;
                    voice.lease = Some(lease);
                }
                Some(Err(_)) => {}
            }
            voices.push(voice);
        }
        Ext::Conditions { idle, game } => f.set_conditions(SystemConditions {
            user_idle: *idle,
            game_mode: *game,
        }),
        Ext::SmallRoster => f.set_roster(small_roster()),
        Ext::BudgetBlock => f.set_background_budget(block()),
        Ext::Kill => drop(f.kill_all()),
        Ext::Restart => {
            voices.clear();
            f.restart().unwrap();
        }
        Ext::CycleAttempt(i) => {
            let a = format!("cykl-{i}-a");
            let b = format!("cykl-{i}-b");
            let base = &sc.tasks[*i].spec;
            let mut x = base.clone();
            x.id = a.clone().into();
            x.parent = None;
            x.window = scheduler_contract::TimeWindow::default();
            x.deps = vec![scheduler_contract::Dependency::on(b.clone())];
            let mut y = x.clone();
            y.id = b.into();
            y.deps = vec![scheduler_contract::Dependency::on(a)];
            match f.submit(vec![x, y]) {
                Err(scheduler_contract::TaskError::Cycle { .. }) => {}
                other => panic!("cykl nie odrzucony: {other:?}"),
            }
        }
    }
}

/// Uruchamia scenariusz do horyzontu i sprawdza niezmienniki.
pub fn run(sc: &Scenario) -> Report {
    let f = FakeScheduler::new();
    let start = f.now_ms();
    let mut report = Report {
        tasks: sc.tasks.len(),
        ..Report::default()
    };
    let mut voices: Vec<Voice> = Vec::new();
    // Zgłoszenia jako zdarzenia (partie o tej samej chwili).
    let mut submits: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (i, t) in sc.tasks.iter().enumerate() {
        submits.entry(t.submit_at).or_default().push(i);
        f.script(&t.spec.id, t.script.clone());
    }
    let mut ext = sc.events.iter().peekable();
    let last_submit = sc.tasks.iter().map(|t| t.submit_at).max().unwrap_or(0);
    let horizon = start + last_submit + 26 * 3_600_000;
    let mut same_instant = 0u32;
    loop {
        let now = f.now_ms();
        let next_submit = submits.keys().next().map(|t| start + t);
        let next_ext = ext.peek().map(|(t, _)| start + t);
        let next_release = voices.iter().map(|v| v.release_at).min();
        let next_core = f.next_event_at();
        let t = [
            next_submit,
            next_ext,
            next_release,
            next_core,
            Some(horizon),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(horizon)
        .max(now);
        same_instant = if t == now { same_instant + 1 } else { 0 };
        if same_instant > 10_000 {
            report.violations.push(format!("brak postępu w t={now}"));
            break;
        }
        f.advance(t - now);
        check_now(&f, &voices, &mut report);
        let now = f.now_ms();
        for v in &mut voices {
            if let Some(fut) = v.fut.as_mut()
                && let Some(res) = poll(fut)
            {
                v.fut = None;
                if let Ok(lease) = res {
                    v.release_at = now + v.hold_ms;
                    v.lease = Some(lease);
                } else {
                    v.release_at = u64::MAX;
                }
            }
            if v.lease.is_some() && !v.counted {
                v.counted = true;
                report.voice_granted += 1;
            }
            if v.release_at <= now {
                v.lease = None;
                v.release_at = u64::MAX;
            }
        }
        voices.retain(|v| v.fut.is_some() || v.lease.is_some());
        if let Some((&at, _)) = submits.iter().next()
            && start + at <= now
        {
            let batch: Vec<_> = submits.remove(&at).unwrap_or_default();
            let specs = batch
                .iter()
                .map(|&i| {
                    let mut s = sc.tasks[i].spec.clone();
                    s.window.not_before_ms = s.window.not_before_ms.map(|nb| now + nb);
                    s.window.deadline_ms = s.window.deadline_ms.map(|d| now + d);
                    s
                })
                .collect();
            if let Err(e) = f.submit(specs) {
                report.violations.push(format!("zgłoszenie odrzucone: {e}"));
            }
        }
        while let Some((at, e)) = ext.peek() {
            if start + at > now {
                break;
            }
            apply(&f, sc, e, &mut voices);
            ext.next();
        }
        check_now(&f, &voices, &mut report);
        if now >= horizon {
            break;
        }
    }
    finish(&f, sc, &voices, &mut report);
    report
}

/// Niezmienniki końcowe: wszystko zakończone z jawnym powodem, kolejność DAG, steering ≤ 1 krok.
fn finish(f: &FakeScheduler, sc: &Scenario, voices: &[Voice], report: &mut Report) {
    for v in f.tasks() {
        if !v.state.is_terminal() {
            report
                .violations
                .push(format!("{} niezakończone: {:?}", v.spec.id, v.state));
        }
    }
    if voices.iter().any(|v| v.fut.is_some()) {
        report
            .violations
            .push("żądanie mowy nierozstrzygnięte".into());
    }
    if !f.held().is_empty() {
        report.violations.push("zasoby trzymane po końcu".into());
    }
    let mut finished: BTreeMap<String, u64> = BTreeMap::new();
    for e in f.events_named(EVENT_FINISHED) {
        let task = e.payload["task"].as_str().unwrap_or_default().to_owned();
        let at = e.payload["at_ms"].as_u64().unwrap_or_default();
        if finished.insert(task.clone(), at).is_some() {
            report
                .violations
                .push(format!("{task} zakończone dwa razy"));
        }
    }
    for t in &sc.tasks {
        if !finished.contains_key(t.spec.id.as_str()) {
            report
                .violations
                .push(format!("{} bez zdarzenia zakończenia", t.spec.id));
        }
    }
    let deps: BTreeMap<&str, Vec<&str>> = sc
        .tasks
        .iter()
        .map(|t| {
            (
                t.spec.id.as_str(),
                t.spec.deps.iter().map(|d| d.task.as_str()).collect(),
            )
        })
        .collect();
    for e in f.events_named(EVENT_DISPATCHED) {
        let task = e.payload["task"].as_str().unwrap_or_default();
        let at = e.payload["at_ms"].as_u64().unwrap_or_default();
        for dep in deps.get(task).into_iter().flatten() {
            match finished.get(*dep) {
                Some(t) if *t <= at => {}
                _ => report
                    .violations
                    .push(format!("{task} wystartowało przed końcem {dep}")),
            }
        }
    }
    for e in f.events_named(EVENT_STEERED) {
        let latency = e.payload["latency_steps"].as_u64().unwrap_or(u64::MAX);
        report.steers_delivered += 1;
        report.max_steer_latency = report.max_steer_latency.max(latency);
        if latency > 1 {
            report
                .violations
                .push(format!("steering po {latency} krokach"));
        }
    }
}
