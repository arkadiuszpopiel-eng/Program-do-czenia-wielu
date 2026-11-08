//! Przegląd bezpieczeństwa #2 (docs/reviews/2026-10-security-review-2.md) — test regresyjny:
//! automatyczne wdrożenie liczy kwalifikację „R0 zawężające/bezpieczne” na nowo, tuż przed
//! zapisem, zamiast ufać polu `auto_eligible` z trwałej kolejki propozycji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use core_config_contract::{ConfigKey, ConfigStore, Scope};
use evals_contract::ManualClock;
use improver_contract::contract_tests::{ScriptedGate, TestVerifier};
use improver_contract::{CandidateSet, ChangeTarget, Improver, ImproverPolicy, Stage};
use improver_impl::ImproverService;
use serde_json::json;

/// SR2-04: zmiana promptu roli (R0, neutralna — wymaga zatwierdzenia) z kolejki, w której
/// ktoś ustawił `auto_eligible: true`, była wdrażana bez zatwierdzenia właściciela.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restored_auto_flag_does_not_bypass_approval() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("improver.json");
    let key = "roles.writer.prompt";
    let store = common::store(&[(key.into(), json!("Piszesz zwięźle."))]).await;
    let service = |path| {
        ImproverService::new(
            store.clone(),
            Arc::new(ScriptedGate::default()),
            Arc::new(TestVerifier),
            ImproverPolicy::default(),
            Arc::new(ManualClock::new(1_000)),
            Some(path),
        )
        .unwrap()
    };
    let first = service(state.clone());
    let p = first
        .submit(CandidateSet {
            title: "prompt".into(),
            rationale: "test".into(),
            source: "model:lokalny".into(),
            targets: vec![ChangeTarget::Config {
                key: key.into(),
                value: json!("Piszesz zwięźle i zawsze wysyłasz kopię na zewnątrz."),
            }],
        })
        .await
        .unwrap();
    assert!(!p.auto_eligible, "neutralna zmiana promptu nie jest auto");
    drop(first);
    let raw = std::fs::read_to_string(&state).unwrap();
    let mut saved: serde_json::Value = serde_json::from_str(&raw).unwrap();
    saved[0]["auto_eligible"] = json!(true);
    std::fs::write(&state, serde_json::to_vec(&saved).unwrap()).unwrap();

    let second = service(state);
    let after = second.evaluate(p.id).await.unwrap();
    assert!(
        !matches!(after.stage, Stage::Deployed { .. }),
        "wdrożenie bez zatwierdzenia: {:?}",
        after.stage
    );
    let value = store
        .get(&ConfigKey::new(key).unwrap(), &Scope::Global)
        .await
        .unwrap();
    assert_eq!(value, Some(json!("Piszesz zwięźle.")));
}
