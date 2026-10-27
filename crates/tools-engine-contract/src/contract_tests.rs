//! Testy kontraktowe: manifesty i argumenty na narzędziach (`-impl` i `-fake`) oraz scenariusz
//! portu (uruchom → zmień → odczytaj → cofnij → zatrzymaj) na atrapie albo prawdziwym silniku.

// Kod testowy (tylko z feature `contract-tests`): nieudane oczekiwanie ma przerwać test.
#![allow(clippy::expect_used)]

use std::sync::Arc;

use serde_json::{Value, json};
use tools_common_contract::{Tool, contract_tests as common};

use crate::{EngineError, EngineMethod, EnginePort, StartOptions, manifests, sample_args};

/// Manifesty, odrzucanie złych argumentów i argumentów spoza zakresu.
pub async fn run_all(tools: &[Arc<dyn Tool>]) {
    assert_eq!(tools.len(), manifests().len());
    for m in manifests() {
        let tool = tools
            .iter()
            .find(|t| t.manifest().name == m.name)
            .unwrap_or_else(|| panic!("brak narzędzia {}", m.name));
        assert_eq!(tool.manifest(), &m);
        common::manifest_is_valid(tool.as_ref());
        common::invalid_args_rejected(tool.as_ref(), "/").await;
        assert_eq!(
            crate::check_args(&m.name, &sample_args(&m.name)),
            Ok(()),
            "{}",
            m.name
        );
    }
    let find = |name: &str| {
        tools
            .iter()
            .find(|t| t.manifest().name == name)
            .unwrap_or_else(|| panic!("{name}"))
            .clone()
    };
    for (name, bad) in [
        ("engine_step", json!({"frames": 0})),
        ("engine_step", json!({"frames": 10_001})),
        ("engine_transaction", json!({"operations": []})),
        (
            "engine_transaction",
            json!({"operations": [{"target_id": 1}]}),
        ),
        (
            "engine_transaction",
            json!({"operations": [{"op": "x"}], "transaction_id": ""}),
        ),
        (
            "engine_raycast",
            json!({"origin": [0, 0, 0], "direction": [0, 0, 0]}),
        ),
        ("engine_inspect", json!({"limit": 0})),
        ("engine_preview", json!({"scene": "demo", "load": "a.json"})),
        ("engine_start", json!({"scene": "demo", "load": "a.json"})),
    ] {
        let out = find(name).call(bad.clone(), &common::ctx("/")).await;
        assert!(!out.is_ok(), "{name}: {bad}");
    }
}

/// Scenariusz portu na pustej scenie: wywołanie przed startem, start, transakcja (idempotentna),
/// odczyt, cofnięcie, kroki, zatrzymanie. Port musi wskazywać działający silnik albo atrapę.
pub async fn port_scenario(port: &dyn EnginePort) {
    let state = port.state();
    assert!(!state.running, "przed startem serwer nie działa");
    let early = port
        .call(EngineMethod::InspectWorldStreaming, json!({}))
        .await;
    assert_eq!(early, Err(EngineError::NotRunning));
    let options = StartOptions {
        paused: true,
        ..StartOptions::default()
    };
    let state = port.start(options.clone()).await.expect("start");
    assert!(state.running && state.paused);
    assert!(
        state
            .url
            .as_deref()
            .is_some_and(|u| u.starts_with("ws://127.0.0.1:"))
    );
    assert!(matches!(
        port.start(options).await,
        Err(EngineError::AlreadyRunning(_))
    ));

    let tx = json!({
        "transaction_id": "alfa-contract-1",
        "save_ram_snapshot": true,
        "operations": [{"op": "entity.spawn", "entity": {
            "tag": "alfa-kostka",
            "transform": {"position": [0.0, 1.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0],
                          "scale": [1.0, 1.0, 1.0]}}}]
    });
    let receipt = port
        .call(EngineMethod::ExecuteTransaction, tx.clone())
        .await
        .expect("transakcja");
    let spawned = receipt["spawned_entities"][0].as_u64().expect("nowa encja");
    let again = port
        .call(EngineMethod::ExecuteTransaction, tx)
        .await
        .expect("ponowienie");
    assert_eq!(
        again, receipt,
        "ten sam identyfikator i treść — ten sam pokwitowanie"
    );

    let scene = port
        .call(EngineMethod::InspectSceneTelemetry, json!({"limit": 16}))
        .await
        .expect("telemetria");
    let ids: Vec<u64> = scene["entities"]
        .as_array()
        .map(|a| a.iter().filter_map(|e| e["id"].as_u64()).collect())
        .unwrap_or_default();
    assert!(ids.contains(&spawned), "nowa encja w telemetrii");

    let conflict = port
        .call(
            EngineMethod::ExecuteTransaction,
            json!({"transaction_id": "alfa-contract-2", "expected_revision": 0,
                   "operations": [{"op": "entity.despawn", "target_id": spawned}]}),
        )
        .await;
    assert!(
        matches!(&conflict, Err(EngineError::Rpc { kind, .. }) if kind == "revision_conflict"),
        "{conflict:?}"
    );

    port.call(EngineMethod::RollbackLastChange, json!({}))
        .await
        .expect("cofnięcie");
    let after = port
        .call(EngineMethod::InspectSceneTelemetry, json!({"limit": 16}))
        .await
        .expect("telemetria po cofnięciu");
    assert!(
        after["entities"]
            .as_array()
            .is_none_or(|a| a.iter().all(|e| e["id"].as_u64() != Some(spawned))),
        "cofnięcie usuwa encję"
    );
    let step: Value = port
        .call(EngineMethod::HeadlessStep, json!({"frames": 3}))
        .await
        .expect("kroki");
    assert_eq!(step["steps"].as_u64(), Some(3));

    assert_eq!(port.stop().await, Ok(true));
    assert_eq!(port.stop().await, Ok(false));
    assert!(!port.state().running);
}
