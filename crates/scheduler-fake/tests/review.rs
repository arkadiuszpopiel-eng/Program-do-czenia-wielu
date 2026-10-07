//! Przegląd bezpieczeństwa #2 (docs/reviews/2026-10-security-review-2.md) — test regresyjny:
//! podzadanie z wykonania (`StepGate::spawn`, decyzja agentki) nie celuje w most CLI, nawet gdy
//! zadanie-rodzic pochodzi od użytkownika — most startuje tylko z jawnego polecenia właściciela.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use agent_backends_contract::BridgeKind;
use scheduler_contract::contract_tests::Script;
use scheduler_contract::{
    Assignee, DispatchId, ExecutorKind, Scheduler, TaskClass, TaskError, TaskOrigin, TaskSpec,
};
use scheduler_fake::FakeScheduler;

/// SR2-08: pochodzenie `User` dziedziczone przez podzadanie pozwalało agentce (np. po
/// wstrzyknięciu w treści czytanej w zadaniu użytkownika) uruchomić Claude Code/Codex.
#[test]
fn spawned_subtask_cannot_target_bridge_even_under_user_task() {
    let f = FakeScheduler::new();
    f.script(&"porzadki".into(), Script::ok(3, 100));
    let parent = TaskSpec::new(
        "porzadki",
        "Porządki",
        Assignee::AnyAgent,
        TaskClass::User,
        TaskOrigin::User,
    );
    f.submit(vec![parent]).unwrap();
    f.advance(50);
    let dispatch: DispatchId = f.core().running()[0].1;
    let mut child = TaskSpec::new(
        "pod-most",
        "Podzadanie",
        Assignee::AnyAgent,
        TaskClass::User,
        TaskOrigin::User,
    );
    child.executor = ExecutorKind::Bridge(BridgeKind::ClaudeCode);
    let err = f.spawn(dispatch, vec![child]).unwrap_err();
    assert!(matches!(err, TaskError::BridgeNotAllowed { .. }), "{err:?}");
}
