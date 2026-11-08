//! ACC-F5-agent-runtime-04 (F5-02): sterowanie uwzględnione w ≤ 1 kroku atomowym — 20/20 prób:
//! 10 tekstem (`AgentRuntime::steer` w losowych chwilach: w trakcie tury modelu, w trakcie
//! zapisu, między krokami) i 10 głosem (scheduler: `StepGate::boundary` → `Continue{steering}`
//! z `SteerVia::Voice` w kolejnych punktach atomowych). Miara: liczba kroków rozpoczętych po
//! wysłaniu, zanim agentka przyjęła wiadomość (`steps_before_delivery`), następny krok to tura
//! modelu, a jej żądanie zawiera treść wiadomości.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_runtime_contract::{
    AgentRuntime, MemCheckpointStore, RunEvent, RunEventEnvelope, RunOptions, Steer, StepKind,
    steps_before_delivery,
};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps, RuntimeExecutor, task_run_id};
use common::answer;
use common::v1::{Routed, Slow, TestGate, dispatch, multi_call, spec_for};
use scheduler_contract::{StepDirective, TaskExecutor, TaskId, TaskOrigin};
use serde_json::json;
use tools_common_contract::Tool;
use tools_fs_contract::FsToolKind;

fn world() -> (Routed, Arc<Runtime>) {
    let p = Routed::new(&["m-delta"]);
    let spans = Arc::new(Mutex::new(Vec::new()));
    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(Slow::new(FsToolKind::Write, 100, spans))];
    let m = p.model("m-delta");
    let writes = |tag: &str, n: usize| {
        let calls: Vec<(String, serde_json::Value)> = (0..n)
            .map(|k| {
                (
                    format!("{tag}{k}"),
                    json!({"path": format!("/u/{tag}{k}.md"), "content": "x"}),
                )
            })
            .collect();
        let refs: Vec<(&str, &str, serde_json::Value)> = calls
            .iter()
            .map(|(id, a)| (id.as_str(), "fs_write", a.clone()))
            .collect();
        multi_call(&refs).delayed(Duration::from_millis(30))
    };
    m.push_script(writes("a", 3));
    m.push_script(writes("b", 2));
    m.push_script(writes("c", 3));
    for _ in 0..6 {
        m.push_script(answer("Koniec.").delayed(Duration::from_millis(30)));
    }
    let rt = Runtime::new(RuntimeDeps {
        provider: Arc::new(p.clone()),
        tools,
        checkpoints: Arc::new(MemCheckpointStore::default()),
        bus: None,
        config: RuntimeConfig::default(),
    });
    (p, Arc::new(rt))
}

/// Sprawdza próbę: ≤ 1 krok przed przyjęciem, następny krok to tura modelu z treścią.
fn check(
    events: &[RunEventEnvelope],
    sent_after: u64,
    text: &str,
    p: &Routed,
    marker: &str,
) -> Result<u32, String> {
    let n = steps_before_delivery(events, sent_after, text).ok_or("nieprzyjęte")?;
    if n > 1 {
        return Err(format!("{n} kroków przed przyjęciem"));
    }
    let at = events
        .iter()
        .position(|e| matches!(&e.event, RunEvent::Steered { message } if message == text))
        .ok_or("brak Steered")?;
    let next = events[at..].iter().find_map(|e| match &e.event {
        RunEvent::StepStarted { kind, .. } => Some(*kind),
        _ => None,
    });
    if matches!(next, Some(StepKind::Tool)) {
        return Err("po sterowaniu ruszyło narzędzie starej tury".into());
    }
    let turns_before = events[..at]
        .iter()
        .filter(
            |e| matches!(&e.event, RunEvent::StepStarted { kind, .. } if *kind != StepKind::Tool),
        )
        .count();
    let req = p
        .model("m-delta")
        .requests()
        .get(turns_before)
        .cloned()
        .ok_or("brak tury po sterowaniu")?;
    let seen = req.messages.iter().any(|m| {
        let t = m.visible_text();
        t.contains(text) && t.contains(marker)
    });
    if !seen {
        return Err("tura po sterowaniu nie zawiera wiadomości".into());
    }
    for r in p.model("m-delta").requests() {
        r.validate().map_err(|e| e.to_string())?;
    }
    Ok(n)
}

async fn text_trial(i: u64) -> Result<u32, String> {
    let (p, rt) = world();
    let run = rt
        .start(spec_for("delta", "operator", &["fs_write"]))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(15 + 55 * i)).await;
    let sent_after = rt.events(&run).unwrap().last().unwrap().seq;
    let text = format!("Korekta {i}: pomiń pliki tymczasowe");
    rt.steer(&run, Steer::Message(text.clone()))
        .map_err(|e| e.to_string())?;
    rt.wait(&run).await.unwrap();
    check(
        &rt.events(&run).unwrap(),
        sent_after,
        &text,
        &p,
        "Wiadomość od właściciela",
    )
}

async fn voice_trial(k: u32) -> Result<u32, String> {
    let (p, rt) = world();
    let gate = TestGate::new();
    let text = format!("Głosem {k}: tylko dokumenty");
    gate.at(
        k,
        StepDirective::Continue {
            steering: vec![common::v1::envelope(
                u64::from(k),
                scheduler_contract::Steer::voice(text.clone()),
            )],
        },
    );
    let task = format!("glos-{k}");
    let run = task_run_id(&TaskId::new(&task), 1);
    let probe_rt = rt.clone();
    let probe_run = run.clone();
    *gate.probe.lock().unwrap() = Some(Box::new(move || {
        probe_rt
            .events(&probe_run)
            .map(|e| e.last().map_or(0, |x| x.seq))
            .unwrap_or(0)
    }));
    let exec = RuntimeExecutor::new(&rt, 40_000);
    let d = dispatch(
        &task,
        1,
        spec_for("delta", "operator", &["fs_write"]),
        RunOptions::default(),
        TaskOrigin::User,
    );
    exec.execute(d, gate.clone()).await;
    let sent_after = *gate
        .marks
        .lock()
        .unwrap()
        .get(&k)
        .ok_or("granica nie wywołana")?;
    check(
        &rt.events(&run).unwrap(),
        sent_after,
        &text,
        &p,
        "Wiadomość głosowa od właściciela",
    )
}

#[tokio::test(start_paused = true)]
async fn steering_within_one_atomic_step_20_of_20() {
    let mut ok = 0;
    let mut failures = Vec::new();
    for i in 0..10 {
        match text_trial(i).await {
            Ok(n) => {
                ok += 1;
                eprintln!("F5-02 tekst #{i}: przyjęte po {n} krokach");
            }
            Err(e) => failures.push(format!("tekst #{i}: {e}")),
        }
    }
    for k in 1..=10 {
        match voice_trial(k).await {
            Ok(n) => {
                ok += 1;
                eprintln!("F5-02 głos (granica {k}): przyjęte po {n} krokach");
            }
            Err(e) => failures.push(format!("głos #{k}: {e}")),
        }
    }
    eprintln!("F5-02: {ok}/20");
    assert_eq!(ok, 20, "{failures:#?}");
}
