//! Generator losowych scenariuszy schedulera (deterministyczny z ziarna: SplitMix64).

use scheduler_contract::contract_tests::{Script, ScriptOutcome};
use scheduler_contract::{
    Assignee, DepCondition, Dependency, Resource, RetryPolicy, TaskClass, TaskOrigin, TaskOutput,
    TaskSpec,
};
use serde_json::json;

/// SplitMix64 — mały, deterministyczny PRNG.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Liczba z `[lo, hi]`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }

    /// Prawda z prawdopodobieństwem `pct` %.
    pub fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }

    pub fn pick<T: Clone>(&mut self, items: &[T]) -> T {
        items[(self.next() % items.len() as u64) as usize].clone()
    }
}

pub fn resources() -> [Resource; 5] {
    [
        Resource::Speaker,
        Resource::Mic,
        Resource::ScreenInput,
        Resource::file("C:\\dane\\a.txt"),
        Resource::file("C:\\dane\\b.txt"),
    ]
}

/// Zadanie scenariusza: specyfikacja, skrypt i chwila zgłoszenia (względna, ms).
pub struct GenTask {
    pub spec: TaskSpec,
    pub script: Script,
    pub submit_at: u64,
}

/// Zdarzenie zewnętrzne.
#[derive(Debug, Clone)]
pub enum Ext {
    Steer(usize, bool),
    Cancel(usize),
    Pause(usize),
    Resume(usize),
    Voice {
        resource: Resource,
        hold_ms: u64,
        max_wait_ms: u64,
    },
    Conditions {
        idle: bool,
        game: bool,
    },
    SmallRoster,
    BudgetBlock,
    Kill,
    Restart,
    CycleAttempt(usize),
}

pub struct Scenario {
    pub tasks: Vec<GenTask>,
    pub events: Vec<(u64, Ext)>,
}

fn assignee(rng: &mut Rng) -> Assignee {
    match rng.range(0, 6) {
        0 => Assignee::Persona("alfa".into()),
        1 => Assignee::Persona("delta".into()),
        2 => Assignee::Role("operator".into()),
        3 => Assignee::Role("critic".into()),
        4 => Assignee::System("pamiec".into()),
        _ => Assignee::AnyAgent,
    }
}

fn condition(rng: &mut Rng) -> DepCondition {
    match rng.range(0, 9) {
        0 => DepCondition::Failed,
        1 | 2 => DepCondition::Finished,
        3 => DepCondition::OutputEquals {
            key: "ok".into(),
            value: json!(rng.chance(50)),
        },
        _ => DepCondition::Succeeded,
    }
}

fn gen_task(rng: &mut Rng, i: usize, earlier: &[u64]) -> GenTask {
    let class = rng.pick(&[TaskClass::Background, TaskClass::Agent, TaskClass::User]);
    let mut spec = TaskSpec::new(
        format!("t{i}"),
        format!("zadanie {i}"),
        assignee(rng),
        class,
        TaskOrigin::User,
    );
    let pool = resources();
    for r in pool {
        if rng.chance(22) {
            spec.resources.push(r);
        }
    }
    if matches!(spec.assignee, Assignee::System(_)) {
        spec.resources.retain(|r| *r != Resource::Speaker);
    }
    if i > 0 {
        for _ in 0..rng.range(0, 2) {
            let d = rng.range(0, i as u64 - 1) as usize;
            if spec.deps.iter().all(|x| x.task.as_str() != format!("t{d}")) {
                spec.deps
                    .push(Dependency::when(format!("t{d}"), condition(rng)));
            }
        }
        if rng.chance(20) {
            spec.parent = Some(format!("t{}", rng.range(0, i as u64 - 1)).into());
        }
    }
    let submit_at = earlier.last().copied().unwrap_or(0) + rng.range(0, 400);
    spec.budget.max_steps = rng.range(1, 10) as u32;
    spec.budget.max_wall_ms = rng.range(100, 4_000);
    if rng.chance(20) {
        spec.budget.max_cost_micro_pln = Some(rng.range(10, 100));
    }
    if class == TaskClass::Background && rng.chance(30) {
        spec.budget.estimated_cost_micro_pln = rng.range(1, 1_000);
    }
    spec.retry = RetryPolicy {
        max_attempts: rng.range(1, 4) as u32,
        initial_backoff_ms: rng.range(1, 300),
        max_backoff_ms: 1_000,
        multiplier: rng.range(1, 3) as u32,
    };
    if rng.chance(25) {
        spec.window.not_before_ms = Some(rng.range(0, 2_000));
    }
    if rng.chance(85) {
        spec.window.deadline_ms = Some(rng.range(2_100, 30_000));
    }
    spec.window.only_when_idle = rng.chance(12);
    spec.window.not_in_game_mode = rng.chance(12);
    let outcome = if rng.chance(15) {
        ScriptOutcome::Fail {
            retryable: rng.chance(60),
        }
    } else {
        ScriptOutcome::Succeed(TaskOutput::text("ok").with("ok", json!(rng.chance(70))))
    };
    let script = Script {
        steps: rng.range(1, 7) as u32,
        step_ms: rng.range(1, 300),
        outcome,
        fail_first: if rng.chance(20) {
            rng.range(1, 3) as u32
        } else {
            0
        },
        hang_at_step: rng.chance(6).then(|| rng.range(1, 3) as u32),
        cost_per_step: rng.range(0, 20),
        fingerprint: rng.chance(10).then_some(7),
    };
    GenTask {
        spec,
        script,
        submit_at,
    }
}

/// Scenariusz z ziarna: 2–12 zadań, 0–14 zdarzeń zewnętrznych.
pub fn scenario(seed: u64) -> Scenario {
    let mut rng = Rng::new(seed);
    let n = rng.range(2, 12) as usize;
    let mut tasks: Vec<GenTask> = Vec::with_capacity(n);
    let mut times = Vec::new();
    for i in 0..n {
        let t = gen_task(&mut rng, i, &times);
        times.push(t.submit_at);
        tasks.push(t);
    }
    let last = times.last().copied().unwrap_or(0);
    let mut events = Vec::new();
    for _ in 0..rng.range(0, 14) {
        let at = rng.range(0, last + 3_000);
        let idx = rng.range(0, n as u64 - 1) as usize;
        let ext = match rng.range(0, 21) {
            0..=3 => Ext::Steer(idx, rng.chance(50)),
            4 | 5 => Ext::Cancel(idx),
            6 | 7 => Ext::Pause(idx),
            8 | 9 => Ext::Resume(idx),
            10..=13 => Ext::Voice {
                resource: rng.pick(&[Resource::Speaker, Resource::Mic, Resource::ScreenInput]),
                hold_ms: rng.range(1, 1_500),
                max_wait_ms: rng.range(0, 3_000),
            },
            14 | 15 => Ext::Conditions {
                idle: rng.chance(60),
                game: rng.chance(30),
            },
            16 => Ext::SmallRoster,
            17 => Ext::BudgetBlock,
            18 => Ext::Restart,
            19 => Ext::CycleAttempt(idx),
            _ if rng.chance(30) => Ext::Kill,
            _ => Ext::Steer(idx, true),
        };
        events.push((at, ext));
    }
    events.sort_by_key(|(t, _)| *t);
    Scenario { tasks, events }
}

/// Scenariusz F5-01: 4–10 zadań zgłoszonych prawie naraz przez różne agentki, często na
/// ekranie i głośniku, z mową użytkownika w tle (sprawdzenie wyłączności przy równoległości).
pub fn scenario_parallel(seed: u64) -> Scenario {
    let mut rng = Rng::new(seed ^ 0xF501);
    let n = rng.range(4, 10) as usize;
    let mut tasks = Vec::with_capacity(n);
    for i in 0..n {
        let mut t = gen_task(&mut rng, i, &[]);
        t.submit_at = rng.range(0, 100);
        t.spec.parent = None;
        t.spec.deps.clear();
        t.spec.window = scheduler_contract::TimeWindow {
            deadline_ms: Some(60_000),
            ..Default::default()
        };
        t.spec.assignee = rng.pick(&[
            Assignee::Persona("alfa".into()),
            Assignee::Persona("beta".into()),
            Assignee::Persona("gama".into()),
            Assignee::Persona("delta".into()),
            Assignee::AnyAgent,
        ]);
        t.spec.resources.clear();
        if rng.chance(50) {
            t.spec.resources.push(Resource::ScreenInput);
        }
        if rng.chance(40) {
            t.spec.resources.push(Resource::Speaker);
        }
        t.spec.budget.max_steps = 20;
        t.spec.budget.max_wall_ms = 20_000;
        t.script.steps = rng.range(2, 6) as u32;
        t.script.step_ms = rng.range(50, 300);
        t.script.hang_at_step = None;
        tasks.push(t);
    }
    tasks.sort_by_key(|t| t.submit_at);
    for (i, t) in tasks.iter_mut().enumerate() {
        t.spec.id = format!("t{i}").into();
        t.spec.title = format!("zadanie {i}");
    }
    let mut events = Vec::new();
    for _ in 0..rng.range(0, 4) {
        events.push((
            rng.range(0, 1_500),
            Ext::Voice {
                resource: rng.pick(&[Resource::Speaker, Resource::ScreenInput, Resource::Mic]),
                hold_ms: rng.range(10, 500),
                max_wait_ms: rng.range(100, 3_000),
            },
        ));
    }
    events.sort_by_key(|(t, _)| *t);
    Scenario { tasks, events }
}
