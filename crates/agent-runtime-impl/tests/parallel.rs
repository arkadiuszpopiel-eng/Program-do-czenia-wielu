//! v1: trzy agentki równolegle na atrapach (wirtualny zegar) — każda z własną rolą, budżetem
//! i taintem; zapisy tych samych plików nigdy nie zachodzą na siebie (dzierżawy
//! `scheduler-lite`), bez dzierżaw kolizje są wykrywane (kontrola negatywna); 100 losowych
//! scenariuszy bez kolizji; odczyty jednej tury równolegle, zapisy szeregowo.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use agent_runtime_contract::{
    AgentRuntime, BudgetKind, MemCheckpointStore, RunEvent, RunOutcome, contract_tests_v1 as ct1,
};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps, RuntimeExt};
use common::v1::{Routed, Slow, Span, collisions, multi_call, spec_for};
use common::{answer, call};
use safety_broker_contract::TaintSource;
use scheduler_contract::{Resource, SchedulerLite};
use scheduler_lite_fake::FakeScheduler;
use serde_json::json;
use tools_common_contract::{Tool, ToolOutcome};
use tools_fs_contract::FsToolKind;

const AGENTS: [&str; 3] = ["alfa", "beta", "delta"];

fn runtime(
    provider: &Routed,
    spans: &Arc<Mutex<Vec<Span>>>,
    locks: Option<Arc<FakeScheduler>>,
) -> Runtime {
    let read = Slow::new(FsToolKind::Read, 50, spans.clone())
        .with_outcome(ToolOutcome::ok("treść z pliku", json!({})).untrusted(TaintSource::File));
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(Slow::new(FsToolKind::Write, 100, spans.clone())),
        Arc::new(read),
    ];
    Runtime::with_ext(
        RuntimeDeps {
            provider: Arc::new(provider.clone()),
            tools,
            checkpoints: Arc::new(MemCheckpointStore::default()),
            bus: None,
            config: RuntimeConfig::default(),
        },
        RuntimeExt {
            locks: locks.map(|l| l as Arc<dyn SchedulerLite>),
            autonomy: None,
        },
    )
}

fn writes(p: &Routed, persona: &str, paths: &[&str]) {
    let model = p.model(&format!("m-{persona}"));
    for (i, path) in paths.iter().enumerate() {
        model.push_script(call(
            &format!("{persona}{i}"),
            "fs_write",
            json!({"path": path, "content": format!("{persona}{i}")}),
        ));
    }
    model.push_script(answer(&format!("{persona}: gotowe")));
}

fn models() -> Routed {
    Routed::new(&["m-alfa", "m-beta", "m-delta"])
}

/// Scenariusz: wspólny plik, odczyt niezaufanej treści u Bety, mały budżet Delty.
async fn scenario(
    locks: Option<Arc<FakeScheduler>>,
) -> (Runtime, Vec<Span>, Vec<agent_runtime_contract::RunId>) {
    let p = models();
    let spans = Arc::new(Mutex::new(Vec::new()));
    writes(&p, "alfa", &["/u/shared.md", "/u/a.md", "/u/shared.md"]);
    p.model("m-beta")
        .push_script(call("b-r", "fs_read", json!({"path": "/u/r.txt"})));
    writes(&p, "beta", &["/u/shared.md", "/u/b.md"]);
    writes(&p, "delta", &["/u/a.md", "/u/shared.md", "/u/shared.md"]);
    let rt = runtime(&p, &spans, locks);
    let mut runs = Vec::new();
    for persona in AGENTS {
        let mut s = spec_for(persona, "operator", &["fs_write", "fs_read"]);
        if persona == "delta" {
            s.budget.max_steps = 4;
        }
        runs.push(rt.start(s).await.unwrap());
    }
    for r in &runs {
        rt.wait(r).await.unwrap();
    }
    let spans = spans.lock().unwrap().clone();
    (rt, spans, runs)
}

#[tokio::test(start_paused = true)]
async fn three_agents_in_parallel_without_resource_collisions() {
    let locks = Arc::new(FakeScheduler::new());
    let (rt, spans, runs) = scenario(Some(locks.clone())).await;
    assert_eq!(collisions(&spans), 0, "{spans:#?}");
    assert!(spans.iter().filter(|s| s.path == "/u/shared.md").count() >= 4);
    let outcomes: Vec<RunOutcome> = runs
        .iter()
        .map(|r| rt.report(r).unwrap().outcome.unwrap())
        .collect();
    assert!(matches!(outcomes[0], RunOutcome::Completed { .. }));
    assert!(matches!(outcomes[1], RunOutcome::Completed { .. }));
    assert_eq!(
        outcomes[2],
        RunOutcome::BudgetExceeded {
            budget: BudgetKind::Steps
        },
        "budżet Delty nie wpływa na inne"
    );
    let tainted: Vec<bool> = runs
        .iter()
        .map(|r| {
            rt.events(r)
                .unwrap()
                .iter()
                .any(|e| matches!(e.event, RunEvent::Tainted { .. }))
        })
        .collect();
    assert_eq!(tainted, vec![false, true, false], "taint tylko u Bety");
    assert!(
        locks.holder(&Resource::file("/u/shared.md")).is_none(),
        "dzierżawy zwolnione"
    );
    let holders: std::collections::BTreeSet<String> = locks
        .requests()
        .iter()
        .map(|r| r.holder.to_string())
        .collect();
    assert_eq!(
        holders.len(),
        3,
        "posiadaczka dzierżaw per przebieg: {holders:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn without_locks_collisions_are_detected() {
    let (_rt, spans, _) = scenario(None).await;
    assert!(
        collisions(&spans) > 0,
        "kontrola negatywna: test wykrywa kolizje"
    );
}

#[tokio::test(start_paused = true)]
async fn hundred_random_scenarios_have_no_collisions() {
    let mut seed: u64 = 0x5eed_f501;
    let mut next = move |n: u64| {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % n
    };
    let paths = ["/u/x.md", "/u/y.md", "/u/z.md"];
    for scenario in 0..100 {
        let p = models();
        let spans = Arc::new(Mutex::new(Vec::new()));
        for persona in AGENTS {
            let n = 1 + next(3) as usize;
            let chosen: Vec<&str> = (0..n).map(|_| paths[next(3) as usize]).collect();
            writes(&p, persona, &chosen);
        }
        let locks = Arc::new(FakeScheduler::new());
        let rt = runtime(&p, &spans, Some(locks));
        let mut runs = Vec::new();
        for persona in AGENTS {
            runs.push(
                rt.start(spec_for(persona, "operator", &["fs_write"]))
                    .await
                    .unwrap(),
            );
        }
        for r in &runs {
            assert!(matches!(
                rt.wait(r).await.unwrap(),
                RunOutcome::Completed { .. }
            ));
        }
        let spans = spans.lock().unwrap().clone();
        assert_eq!(collisions(&spans), 0, "scenariusz {scenario}: {spans:#?}");
    }
}

#[tokio::test(start_paused = true)]
async fn reads_in_one_turn_run_in_parallel_writes_serially() {
    let p = models();
    let spans = Arc::new(Mutex::new(Vec::new()));
    p.model("m-alfa").push_script(multi_call(&[
        ("r1", "fs_read", json!({"path": "/u/1"})),
        ("r2", "fs_read", json!({"path": "/u/2"})),
        ("w1", "fs_write", json!({"path": "/u/3", "content": "x"})),
        ("r3", "fs_read", json!({"path": "/u/4"})),
    ]));
    p.model("m-alfa").push_script(answer("koniec"));
    let rt = runtime(&p, &spans, None);
    let t0 = tokio::time::Instant::now();
    let run = rt
        .start(spec_for("alfa", "operator", &["fs_write", "fs_read"]))
        .await
        .unwrap();
    rt.wait(&run).await.unwrap();
    let s = spans.lock().unwrap().clone();
    let at = |path: &str| s.iter().find(|x| x.path == path).unwrap().clone();
    assert_eq!(at("/u/1").start, at("/u/2").start, "odczyty równolegle");
    assert!(
        at("/u/3").start >= at("/u/1").end,
        "zapis po paczce odczytów"
    );
    assert!(
        at("/u/4").start >= at("/u/3").end,
        "kolejny odczyt po zapisie"
    );
    assert_eq!(
        t0.elapsed().as_millis(),
        200,
        "50 (paczka) + 100 (zapis) + 50"
    );
    for r in p.model("m-alfa").requests() {
        r.validate().unwrap();
    }
    let report = rt.report(&run).unwrap();
    assert_eq!(report.done.len(), 4);
    ct1::report_matches_events(&rt, &run, &report);
}

#[tokio::test(start_paused = true)]
async fn contract_v1_on_impl() {
    let p = models();
    for persona in AGENTS {
        p.model(&format!("m-{persona}"))
            .push_script(answer("gotowe"));
    }
    p.model("m-alfa").push_script(answer("gotowe"));
    let spans = Arc::new(Mutex::new(Vec::new()));
    let rt = runtime(&p, &spans, None);
    ct1::default_options_behave_like_v0(&rt, spec_for("alfa", "operator", &[])).await;
    let specs = AGENTS
        .iter()
        .map(|a| spec_for(a, "operator", &["fs_read"]))
        .collect();
    ct1::parallel_runs_are_isolated(&rt, specs).await;
}
