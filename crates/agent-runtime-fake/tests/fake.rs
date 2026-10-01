//! Atrapa przechodzi testy kontraktowe; skrypt z prośbą o zatwierdzenie, pauza/wznowienie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use agent_runtime_contract::contract_tests as ct;
use agent_runtime_contract::{
    AgentRuntime, RunError, RunEvent, RunOutcome, RunStatus, Steer, StepKind,
};
use agent_runtime_fake::{FakeAgentRuntime, RunScript};
use safety_broker_contract::ApprovalId;

fn done() -> RunScript {
    RunScript::new()
        .then(
            100,
            RunEvent::Planned {
                text: "Plan".into(),
            },
        )
        .then(
            50,
            RunEvent::StepStarted {
                step: 1,
                kind: StepKind::Tool,
                tool: Some("fs_move".into()),
                input: "a → b".into(),
            },
        )
        .then(
            10,
            RunEvent::WaitingApproval {
                step: 1,
                approval: ApprovalId(7),
                explanation: "poza zakresem".into(),
            },
        )
        .finish(
            500,
            RunOutcome::Completed {
                summary: "Gotowe".into(),
                verified: Some(true),
            },
        )
}

#[tokio::test(start_paused = true)]
async fn contract_suite() {
    let rt = FakeAgentRuntime::new();
    rt.push_script(done());
    ct::finishing_run_is_consistent(&rt, ct::sample_spec("m", &[])).await;
    ct::long_run_steer_and_cancel(&rt, ct::sample_spec("m", &[])).await;
    ct::errors(&rt, ct::sample_spec("m", &[])).await;
}

#[tokio::test(start_paused = true)]
async fn approval_card_and_pause() {
    let rt = FakeAgentRuntime::new();
    rt.set_default(done());
    let run = rt.start(ct::sample_spec("m", &[])).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(170)).await;
    assert_eq!(
        rt.status(&run).unwrap(),
        RunStatus::WaitingApproval {
            step: 1,
            approval: ApprovalId(7)
        }
    );
    rt.steer(&run, Steer::PauseAfterCurrent).unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    assert!(matches!(rt.status(&run).unwrap(), RunStatus::Paused { .. }));
    rt.steer(&run, Steer::Resume).unwrap();
    assert!(matches!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::Completed { .. }
    ));
    assert_eq!(rt.steers().len(), 2);
    assert_eq!(
        rt.resume(&run).await,
        Err(RunError::AlreadyFinished(run.clone()))
    );
}
