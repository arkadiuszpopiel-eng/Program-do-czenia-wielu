//! Wskazówki trasy dla Routera (dobór modelu według trudności, zgłoszenie właściciela
//! 2026-10-14): trudność z celu przebiegu, Krytyczka na szczeblu sprawdzania, po odrzuceniu
//! odpowiedzi poprawka od modelu mocniejszego niż autor, lżejsza Krytyczka bez werdyktu → druga
//! próba szczebel wyżej, delegacja z klasą roli wykonawczyni.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use agent_runtime_contract::{
    AgentRuntime, MemCheckpointStore, RunEvent, RunOptions, RunOutcome, RunSpec,
};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use common::v1::{Routed, Slow, spec_for, standard_crew};
use common::{answer, call};
use providers_fake::Script;
use serde_json::json;
use tools_common_contract::Tool;
use tools_fs_contract::FsToolKind;

/// Odpowiedź końcowa „napisana” przez model zakwalifikowany przez Router (`dostawca:model`).
fn answer_by(model: &str, text: &str) -> Script {
    Script::text(model, &[text])
}

fn world() -> (Routed, Runtime) {
    world_with(RuntimeConfig::default())
}

fn world_with(config: RuntimeConfig) -> (Routed, Runtime) {
    let p = Routed::new(&["m-delta", "m-gama", "m-alfa"]);
    let spans = Arc::new(Mutex::new(Vec::new()));
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(Slow::new(FsToolKind::Write, 10, spans.clone())),
        Arc::new(Slow::new(FsToolKind::Read, 10, spans)),
    ];
    let rt = Runtime::new(RuntimeDeps {
        provider: Arc::new(p.clone()),
        tools,
        checkpoints: Arc::new(MemCheckpointStore::default()),
        bus: None,
        config,
    });
    (p, rt)
}

fn author() -> RunSpec {
    let mut s = spec_for("delta", "coder", &["fs_read", "fs_write"]);
    s.verify = true;
    s.goal = "Zaimplementuj parser CSV i dodaj testy".into();
    s
}

fn crew() -> RunOptions {
    RunOptions {
        crew: Some(standard_crew(&[
            ("critic", "m-gama"),
            ("operator", "m-delta"),
        ])),
        ..RunOptions::default()
    }
}

fn notes(rt: &Runtime, run: &agent_runtime_contract::RunId) -> Vec<(bool, String)> {
    rt.events(run)
        .unwrap()
        .iter()
        .filter_map(|e| match &e.event {
            RunEvent::Verified { ok, note } => Some((*ok, note.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn rejected_answer_is_fixed_by_a_stronger_model_and_checked_on_the_review_rung() {
    let (p, rt) = world();
    let d = p.model("m-delta");
    d.push_script(answer_by("local:qwen3-27b", "Parser gotowy."));
    d.push_script(answer_by(
        "anthropic:claude-opus-5-5",
        "Poprawiony parser z testami.",
    ));
    let g = p.model("m-gama");
    g.push_script(answer_by(
        "local-light:bielik-4.5b",
        "WERYFIKACJA: BŁĄD — brak testów.",
    ));
    g.push_script(answer_by("local-light:bielik-4.5b", "WERYFIKACJA: OK"));
    let run = rt.start_with(author(), crew()).await.unwrap();
    assert_eq!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::Completed {
            summary: "Poprawiony parser z testami.".into(),
            verified: Some(true)
        }
    );
    let first = &d.requests()[0];
    assert_eq!(
        first.meta.route.task.as_deref(),
        Some("Zaimplementuj parser CSV i dodaj testy"),
        "trudność z celu przebiegu"
    );
    assert!(!first.meta.route.review && first.meta.route.escalate_from.is_none());
    let fix = &d.requests()[1];
    assert_eq!(
        fix.meta.route.escalate_from.as_deref(),
        Some("local:qwen3-27b"),
        "poprawka od modelu mocniejszego niż autor odrzuconej odpowiedzi"
    );
    for check in g.requests() {
        assert!(check.meta.route.review, "Krytyczka na szczeblu sprawdzania");
        assert!(check.meta.route.task.is_none());
    }
    let v = notes(&rt, &run);
    assert_eq!(v.len(), 2);
    assert!(
        v[0].1
            .contains("poprawka trafi do mocniejszego modelu niż local:qwen3-27b"),
        "{}",
        v[0].1
    );
    assert!(
        !v[1].1.contains("poprawka trafi"),
        "zaakceptowana — bez podniesienia"
    );
}

#[tokio::test(start_paused = true)]
async fn light_critic_without_verdict_gets_a_second_try_one_rung_up() {
    let (p, rt) = world();
    p.model("m-delta")
        .push_script(answer_by("openai:gpt-6-luna", "Gotowe."));
    let g = p.model("m-gama");
    g.push_script(answer_by(
        "local-light:bielik-4.5b",
        "Nie wiem, trudno powiedzieć.",
    ));
    g.push_script(answer_by(
        "openai:gpt-6-luna",
        "WERYFIKACJA: OK — parser i testy są.",
    ));
    let run = rt.start_with(author(), crew()).await.unwrap();
    assert_eq!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::Completed {
            summary: "Gotowe.".into(),
            verified: Some(true)
        }
    );
    let checks = g.requests();
    assert_eq!(checks.len(), 2);
    assert!(checks[0].meta.route.escalate_from.is_none());
    assert_eq!(
        checks[1].meta.route.escalate_from.as_deref(),
        Some("local-light:bielik-4.5b"),
        "druga Krytyczka szczebel wyżej niż pierwsza"
    );
    assert!(checks[1].meta.route.review);
    assert_eq!(rt.children(&run).unwrap().len(), 2);
    assert_eq!(
        notes(&rt, &run),
        vec![(
            true,
            "Gama (Krytyczka): WERYFIKACJA: OK — parser i testy są.".to_owned()
        )]
    );
}

#[tokio::test(start_paused = true)]
async fn persistent_rejection_escalates_each_round() {
    let (p, rt) = world_with(RuntimeConfig {
        max_verify_rounds: 3,
        ..RuntimeConfig::default()
    });
    let d = p.model("m-delta");
    d.push_script(answer_by("local-light:bielik-4.5b", "v1"));
    d.push_script(answer_by("local:qwen3-27b", "v2"));
    d.push_script(answer_by("anthropic:claude-opus-5-5", "v3"));
    let g = p.model("m-gama");
    for _ in 0..3 {
        g.push_script(answer_by(
            "local-light:bielik-4.5b",
            "WERYFIKACJA: BŁĄD — źle.",
        ));
    }
    let run = rt.start_with(author(), crew()).await.unwrap();
    let outcome = rt.wait(&run).await.unwrap();
    let froms: Vec<Option<String>> = d
        .requests()
        .iter()
        .map(|r| r.meta.route.escalate_from.clone())
        .collect();
    assert_eq!(
        froms,
        vec![
            None,
            Some("local-light:bielik-4.5b".to_owned()),
            Some("local:qwen3-27b".to_owned())
        ],
        "każda odrzucona runda — od modelu, który ją napisał"
    );
    assert!(matches!(
        outcome,
        RunOutcome::Completed {
            verified: Some(false),
            ..
        }
    ));
}

#[tokio::test(start_paused = true)]
async fn delegation_carries_the_role_class_and_its_own_goal() {
    let (p, rt) = world();
    p.model("m-alfa").push_script(call(
        "d1",
        "delegate_task",
        json!({"role": "operator", "goal": "Zapisz raport do /Users/ala/Documents/r.md"}),
    ));
    p.model("m-alfa")
        .push_script(answer("Delta zapisała raport."));
    p.model("m-delta").push_script(answer("Zapisałam raport."));
    let spec = spec_for("alfa", "conductor", &["fs_read", "fs_write"]);
    let run = rt.start_with(spec, crew()).await.unwrap();
    assert!(matches!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::Completed { .. }
    ));
    let child = &p.model("m-delta").requests()[0];
    assert_eq!(child.meta.route.class.as_deref(), Some("gui-vision"));
    assert_eq!(
        child.meta.route.task.as_deref(),
        Some("Zapisz raport do /Users/ala/Documents/r.md")
    );
    let parent = &p.model("m-alfa").requests()[0];
    assert!(
        parent.meta.route.class.is_none(),
        "rodzic — klasa własnego dostawcy"
    );
}

/// Regresja próby obowiązków 0.0.51: bez mocniejszego modelu nie ma drugiej Krytyczki ani
/// podniesienia — ponowienie tym samym modelem tylko wydłużało pracę (wieczór > 10 min).
#[tokio::test(start_paused = true)]
async fn without_a_stronger_model_there_is_no_retry_and_no_escalation() {
    let (p, rt) = world();
    let d = p.model("m-delta");
    d.push_script(answer_by("local:jedyny-27b", "v1"));
    d.push_script(answer_by("local:jedyny-27b", "v2"));
    let g = p.model("m-gama");
    g.push_script(answer_by("local:jedyny-27b", "Nie wiem."));
    g.push_script(answer_by(
        "local:jedyny-27b",
        "WERYFIKACJA: BŁĄD — brak testów.",
    ));
    let run = rt.start_with(author(), crew()).await.unwrap();
    let _ = rt.wait(&run).await.unwrap();
    // Pierwsza Krytyczka bez werdyktu — bez drugiej próby (jest tylko ten jeden model).
    assert_eq!(g.requests().len(), 1, "jedna próba Krytyczki");
    assert!(
        d.requests()
            .iter()
            .all(|r| r.meta.route.escalate_from.is_none()),
        "bez podniesienia"
    );
}
