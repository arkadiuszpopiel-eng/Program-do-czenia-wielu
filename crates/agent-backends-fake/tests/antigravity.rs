//! Most Antigravity (`agy`) z fałszywym CLI: bez kanału zgód i bez sterowania w trakcie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use agent_backends_contract::contract_tests::scenario::Scenario;
use agent_backends_contract::contract_tests::{RecordingSink, assert_grammar, collect};
use agent_backends_contract::{AgentBackend, AgentEvent, BackendError, BridgeKind, TaskSpec};
use core_bus_contract::SessionId;

const LIMIT: Duration = Duration::from_secs(60);

fn spec(bridge: BridgeKind, scenario: &Scenario, source: &std::path::Path) -> TaskSpec {
    TaskSpec::user_request(bridge, scenario.prompt(), source, SessionId::new("test"))
}

/// Antigravity nie ma kanału zgód: polecenia powłoki odrzuca sam (krok `denied`), do Alfy nie
/// trafia żadna prośba o zgodę, a dosłanie wiadomości w trakcie jest odrzucane wprost.
#[tokio::test(flavor = "multi_thread")]
async fn antigravity_refuses_shell_itself_and_is_not_steerable() {
    let sink = RecordingSink::with_pattern(vec![true]);
    let h = common::harness(sink.clone(), |_| {}).await;
    let handle = h
        .backend
        .submit_task(spec(
            BridgeKind::Antigravity,
            &Scenario::Permission(2),
            &h.source,
        ))
        .await
        .unwrap();
    let steer = h.backend.steer(&handle.task, "zmień plan".into()).await;
    assert!(
        matches!(
            steer,
            Err(BackendError::SteeringUnavailable(_) | BackendError::TaskFinished)
        ),
        "{steer:?}"
    );
    let events = collect(&h.backend, &handle.task, LIMIT).await;
    assert_grammar(&events);
    assert!(sink.seen().is_empty(), "agy nie pyta Alfy o zgodę");
    let denied = events
        .iter()
        .filter(|e| matches!(&e.event, AgentEvent::ToolFinished { is_error: true, .. }))
        .count();
    assert_eq!(denied, 2);
    assert!(
        matches!(&events.last().unwrap().event, AgentEvent::Done { result } if result.text == "decisions=deny,deny")
    );
}
