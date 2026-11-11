//! Raport odmowy pochodzi z wykonania narzędzia, także gdy model pomija powód lub po restarcie.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use agent_runtime_contract::{
    AgentRuntime, Checkpoint, CheckpointStore, RunId, RunOutcome, StepStatus,
};
use common::{answer, call, spec, world};
use risk_classifier_contract::KernelRule;
use serde_json::json;
use tools_common_contract::{DenialReason, ToolOutcome};
use tools_fs_contract::FsToolKind;

fn summary(outcome: RunOutcome) -> String {
    match outcome {
        RunOutcome::Completed { summary, .. } => summary,
        other => panic!("oczekiwano zakończonego raportu: {other:?}"),
    }
}

#[tokio::test(start_paused = true)]
async fn policy_denial_cannot_disappear_from_the_final_report() {
    let w = world();
    w.fs.push(
        FsToolKind::Write,
        ToolOutcome::denied(DenialReason::Policy, "zapis main.rs"),
    );
    w.provider.push_script(call(
        "write",
        "fs_write",
        json!({"path":"main.rs","content":"fn main() {}"}),
    ));
    // Rzeczywisty Qwen w sondzie poza runtime pomijał odmowę i przekazywał poprawkę użytkownikowi.
    w.provider
        .push_script(answer("Popraw kod, aby zwracać 42 jako liczbę."));
    let run = w.runtime.start(spec()).await.unwrap();
    let result = summary(w.runtime.wait(&run).await.unwrap());
    assert_eq!(
        w.runtime.report(&run).unwrap().not_done[0].status,
        StepStatus::Denied
    );
    assert!(
        result.contains("Odmowa narzędzia"),
        "brak informacji o odmowie: {result}"
    );
    assert!(result.contains("1") && result.contains("nie wykonano"));
    assert_eq!(w.runtime.report(&run).unwrap().not_done.len(), 1);
    assert!(w.runtime.report(&run).unwrap().done.is_empty());
}

#[tokio::test(start_paused = true)]
async fn saved_denial_survives_restart_without_exposing_arguments() {
    let w = world();
    let run = RunId::new("denial-before-restart");
    let mut cp = Checkpoint::initial(run.clone(), spec());
    cp.denied_calls
        .insert("fs_write:{\"path\":\"private-marker-not-for-report\"}".into());
    w.store.save(&cp).unwrap();
    w.provider.push_script(answer("Sprawdź kod programu."));
    w.runtime.resume(&run).await.unwrap();
    let result = summary(w.runtime.wait(&run).await.unwrap());
    assert!(
        result.contains("Odmowa narzędzia"),
        "utracono odmowę z checkpointu: {result}"
    );
    assert!(!result.contains("private-marker-not-for-report"));
    assert!(w.fs.calls(FsToolKind::Write).is_empty());
}

#[tokio::test(start_paused = true)]
async fn hard_denial_stops_before_another_model_answer() {
    let w = world();
    w.fs.push(
        FsToolKind::Write,
        ToolOutcome::denied(
            DenialReason::KernelBlock {
                rule: KernelRule::CredentialDenylist,
            },
            "zapis",
        ),
    );
    w.provider.push_script(call(
        "write",
        "fs_write",
        json!({"path":"main.rs","content":"fn main() {}"}),
    ));
    w.provider.push_script(answer("Gotowe."));
    let run = w.runtime.start(spec()).await.unwrap();
    assert_eq!(w.runtime.wait(&run).await.unwrap(), RunOutcome::Refused);
    assert_eq!(
        w.provider.requests().len(),
        1,
        "po twardej odmowie nie ma dalszej odpowiedzi modelu"
    );
    assert_eq!(w.runtime.report(&run).unwrap().not_done.len(), 1);
}
