//! Przypadki kontraktowe potoku: wdrożenie, zatwierdzenia, bramka, warunki, konflikty.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use core_config_contract::{ConfigKey, ConfigLayer, Origin, Scope};
use evals_contract::GateStage;
use serde_json::{Value, json};

use super::{Harness, ScriptedProposer, Setup, signature_for};
use crate::{
    CandidateSet, ChangeTarget, ImproverError, MetricsSnapshot, Ring, RunConditions, Stage,
    UserApproval, Violation,
};

const IDLE: RunConditions = RunConditions {
    on_battery: false,
    game_mode: false,
    user_idle: true,
};

fn set(key: &str, value: Value) -> CandidateSet {
    CandidateSet {
        title: format!("zmiana {key}"),
        rationale: "test".into(),
        source: "test".into(),
        targets: vec![ChangeTarget::Config {
            key: key.into(),
            value,
        }],
    }
}

fn metrics(pass_rate: f64) -> MetricsSnapshot {
    MetricsSnapshot {
        ts_ms: 0,
        metrics: BTreeMap::from([("pass_rate".to_owned(), pass_rate)]),
        observations: Vec::new(),
    }
}

async fn value(h: &Harness, key: &str) -> Option<Value> {
    let key = ConfigKey::new(key).unwrap_or_else(|e| panic!("{e}"));
    h.config
        .get(&key, &Scope::Global)
        .await
        .unwrap_or_else(|e| panic!("{e}"))
}

fn ok<T>(r: Result<T, ImproverError>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// R0 zawężające: piaskownica → holdout → auto-wdrożenie przez core-config; regresja → rollback.
pub(super) async fn r0_auto_deploy_and_regression_rollback<F, Fut>(factory: &F)
where
    F: Fn(Setup) -> Fut,
    Fut: Future<Output = Harness>,
{
    let setup = Setup::new(vec![("memory.recall.top_k".into(), json!(8))]);
    let gate = Arc::clone(&setup.gate);
    let h = factory(setup).await;
    // Punkt odniesienia metryk przed wdrożeniem.
    ok(h.improver.observe(&metrics(0.9), IDLE).await);
    let p = ok(h
        .improver
        .submit(set("memory.recall.top_k", json!(4)))
        .await);
    assert!(p.auto_eligible && p.ring == Ring::R0, "{p:?}");
    assert_eq!(p.stage, Stage::Proposed);
    let p = ok(h.improver.evaluate(p.id).await);
    assert_eq!(p.stage, Stage::Deployed { auto: true });
    assert!(p.sandbox.is_some() && p.holdout.is_some());
    let stages: Vec<GateStage> = gate.requests().iter().map(|r| r.stage).collect();
    assert_eq!(stages, [GateStage::Sandbox, GateStage::Holdout]);
    assert!(
        gate.requests()
            .iter()
            .all(|r| r.repeats >= 5 && r.suite.as_str() == "f8-improver-r0")
    );
    assert_eq!(value(&h, "memory.recall.top_k").await, Some(json!(4)));
    assert_eq!(h.improver_writes(), ["memory.recall.top_k"]);
    // Bez regresji — nic się nie dzieje; regresja → automatyczny rollback i wychładzanie.
    assert!(ok(h.improver.monitor(&metrics(0.89)).await).is_empty());
    let rolled = ok(h.improver.monitor(&metrics(0.7)).await);
    assert_eq!(rolled.len(), 1);
    assert!(matches!(
        rolled[0].stage,
        Stage::RolledBack { auto: true, .. }
    ));
    assert_eq!(value(&h, "memory.recall.top_k").await, Some(json!(8)));
    let again = h
        .improver
        .submit(set("memory.recall.top_k", json!(4)))
        .await;
    assert!(
        matches!(again, Err(ImproverError::Cooldown(_))),
        "{again:?}"
    );
    assert!(!rolled[0].diff_lines().is_empty() && !rolled[0].rollback_plan().is_empty());
    assert!(!(h.events)().is_empty());
}

/// Rozszerzające R0 i R1: nigdy automatycznie; zatwierdzenie musi dotyczyć tego diffu (R1 z podpisem).
pub(super) async fn widening_and_r1_need_valid_approval<F, Fut>(factory: &F)
where
    F: Fn(Setup) -> Fut,
    Fut: Future<Output = Harness>,
{
    let h = factory(Setup::new(vec![("memory.recall.top_k".into(), json!(8))])).await;
    let wide = ok(h
        .improver
        .submit(set("memory.recall.top_k", json!(12)))
        .await);
    assert!(!wide.auto_eligible);
    let wide = ok(h.improver.evaluate(wide.id).await);
    assert_eq!(wide.stage, Stage::AwaitingApproval);
    assert!(h.improver_writes().is_empty());
    let skill = ok(h
        .improver
        .submit(set("skills.raport.playbook", json!("1. zbierz\n2. wyślij")))
        .await);
    let skill = ok(h.improver.evaluate(skill.id).await);
    assert_eq!(
        (skill.ring, &skill.stage),
        (Ring::R1, &Stage::AwaitingApproval)
    );
    let approval = |id, digest: &str, signature: Option<String>| UserApproval {
        proposal: id,
        digest: digest.to_owned(),
        surface: "zdrowie-systemu".into(),
        signature,
    };
    let forged = [
        approval(wide.id, &skill.digest, None),
        approval(skill.id, &skill.digest, None),
        approval(skill.id, &skill.digest, Some(signature_for(&wide.digest))),
        approval(skill.id, &wide.digest, Some(signature_for(&wide.digest))),
    ];
    for a in forged {
        assert!(matches!(
            h.improver.approve(a).await,
            Err(ImproverError::ApprovalInvalid(_))
        ));
    }
    assert!(h.improver_writes().is_empty());
    let wide = ok(h
        .improver
        .approve(approval(wide.id, &wide.digest, None))
        .await);
    assert_eq!(wide.stage, Stage::Deployed { auto: false });
    let skill = ok(h
        .improver
        .approve(approval(
            skill.id,
            &skill.digest,
            Some(signature_for(&skill.digest)),
        ))
        .await);
    assert_eq!(skill.stage, Stage::Deployed { auto: false });
    assert_eq!(
        value(&h, "skills.raport.playbook").await,
        Some(json!("1. zbierz\n2. wyślij"))
    );
    let skill = ok(h.improver.rollback(skill.id).await);
    assert!(matches!(skill.stage, Stage::RolledBack { auto: false, .. }));
    assert_eq!(value(&h, "skills.raport.playbook").await, None);
    assert!(matches!(
        h.improver.rollback(skill.id).await,
        Err(ImproverError::WrongStage { .. })
    ));
}

/// Porażka w piaskownicy albo na holdoucie — brak wdrożenia; odrzucenie przez użytkownika.
pub(super) async fn gate_failures_block_deploy<F, Fut>(factory: &F)
where
    F: Fn(Setup) -> Fut,
    Fut: Future<Output = Harness>,
{
    let setup = Setup::new(vec![
        ("router.weights.local_chat".into(), json!(0.5)),
        ("voice.turn.patience_ms".into(), json!(800)),
    ]);
    setup.gate.fail_sandbox_on("router.weights.local_chat");
    setup.gate.fail_holdout_on("voice.turn.patience_ms");
    let h = factory(setup).await;
    let a = ok(h
        .improver
        .submit(set("router.weights.local_chat", json!(0.55)))
        .await);
    assert!(matches!(
        ok(h.improver.evaluate(a.id).await).stage,
        Stage::SandboxFailed { .. }
    ));
    let b = ok(h
        .improver
        .submit(set("voice.turn.patience_ms", json!(900)))
        .await);
    let b = ok(h.improver.evaluate(b.id).await);
    assert!(matches!(b.stage, Stage::HoldoutFailed { .. }));
    assert!(b.sandbox.is_some());
    assert!(h.improver_writes().is_empty());
    assert_eq!(ok(h.improver.reject(b.id).await).stage, Stage::Rejected);
    assert!(matches!(
        h.improver.evaluate(b.id).await,
        Err(ImproverError::WrongStage { .. })
    ));
}

/// Warunki pracy, konflikt z użytkownikiem, zestaw mieszany odrzucony w całości, limity.
pub(super) async fn conditions_conflicts_and_atomic_sets<F, Fut>(factory: &F)
where
    F: Fn(Setup) -> Fut,
    Fut: Future<Output = Harness>,
{
    let h = factory(Setup::new(vec![("memory.recall.top_k".into(), json!(8))])).await;
    for c in [
        RunConditions {
            on_battery: true,
            ..IDLE
        },
        RunConditions {
            game_mode: true,
            ..IDLE
        },
        RunConditions {
            user_idle: false,
            ..IDLE
        },
    ] {
        assert!(matches!(
            h.improver.observe(&metrics(0.9), c).await,
            Err(ImproverError::NotNow(_))
        ));
    }
    let p = ok(h
        .improver
        .submit(set("memory.recall.top_k", json!(4)))
        .await);
    let key = ConfigKey::new("memory.recall.top_k").unwrap_or_else(|e| panic!("{e}"));
    ok(h.config
        .set(
            &key,
            Some(json!(6)),
            &Scope::Global,
            &ConfigLayer::Shared,
            Origin::User,
        )
        .await
        .map_err(|e| ImproverError::Config(e.to_string())));
    let p = ok(h.improver.evaluate(p.id).await);
    assert!(matches!(p.stage, Stage::Aborted { .. }), "{p:?}");
    assert_eq!(value(&h, "memory.recall.top_k").await, Some(json!(6)));
    let mixed = CandidateSet {
        targets: vec![
            ChangeTarget::Config {
                key: "ui.suggestions.enabled".into(),
                value: json!(false),
            },
            ChangeTarget::Config {
                key: "kernel.autonomy.global".into(),
                value: json!("L4"),
            },
        ],
        ..set("x", json!(0))
    };
    assert!(matches!(
        h.improver.submit(mixed).await,
        Err(ImproverError::Guard(Violation::KernelPolicy(_)))
    ));
    let dup = CandidateSet {
        targets: vec![
            ChangeTarget::Config {
                key: "ui.suggestions.enabled".into(),
                value: json!(false),
            },
            ChangeTarget::Config {
                key: "ui.suggestions.enabled".into(),
                value: json!(false),
            },
        ],
        ..set("x", json!(0))
    };
    assert!(matches!(
        h.improver.submit(dup).await,
        Err(ImproverError::Guard(Violation::InvalidSet(_)))
    ));
    let empty = CandidateSet {
        targets: Vec::new(),
        ..set("x", json!(0))
    };
    assert!(h.improver.submit(empty).await.is_err());
    assert!(h.improver_writes().is_empty());
    assert!(h.improver.blocked().len() >= 3);
    assert!(h.improver.policy().validate().is_ok());
}

/// Model nie podszyje się pod regułę; złośliwe zmiany od modelu zablokowane; kod → szkic zgłoszenia.
pub(super) async fn untrusted_proposer_and_code_drafts<F, Fut>(factory: &F)
where
    F: Fn(Setup) -> Fut,
    Fut: Future<Output = Harness>,
{
    let mut setup = Setup::new(vec![("ui.suggestions.enabled".into(), json!(true))]);
    let evil = CandidateSet {
        source: "rule:pronunciation".into(),
        ..set("improver.auto_deploy_r0", json!(true))
    };
    let fine = CandidateSet {
        source: "rule:pronunciation".into(),
        ..set("ui.suggestions.enabled", json!(false))
    };
    setup
        .proposers
        .push(Arc::new(ScriptedProposer::new(vec![evil, fine])));
    let h = factory(setup).await;
    let created = ok(h.improver.observe(&metrics(0.9), IDLE).await);
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].source, "model:skryptowany");
    let blocked = h.improver.blocked();
    assert!(
        blocked.iter().any(
            |b| b.target == "config:improver.auto_deploy_r0" && b.source == "model:skryptowany"
        )
    );
    let code = CandidateSet {
        targets: vec![ChangeTarget::Code {
            path: "crates/core-bus-impl/src/lib.rs".into(),
            diff: "+ szybciej".into(),
        }],
        ..set("x", json!(0))
    };
    let r = h.improver.submit(code).await;
    assert!(matches!(
        r,
        Err(ImproverError::Guard(Violation::RingNotAllowed {
            ring: Ring::R3,
            ..
        }))
    ));
    assert_eq!(h.improver.issue_drafts().len(), 1);
    assert!(h.improver_writes().is_empty());
}
