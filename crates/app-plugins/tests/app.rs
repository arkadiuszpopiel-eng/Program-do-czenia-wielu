//! Strona „Wtyczki” na prawdziwym runtime (wasmtime) i magazynie w katalogu: propozycja → karta
//! (zdolności, limity, hash, R2) → zatwierdzenie tylko z przejrzanym hashem → narzędzia w rejestrze
//! od razu → wyłączenie/włączenie/usunięcie; trwałość po „restarcie”; problemy dla Diagnosty;
//! błędy wejścia; tryb niedostępny.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::ErrorCode;
use app_api::dto::{PluginOrigin, PluginProblemKind, PluginStateView};
use common::*;
use core_registry_contract::HealthStatus;
use plugin_runtime_contract::samples;
use serde_json::json;

const WORDS: &[u8] = br#"{"words":3}"#;

fn manifest(id: &str, version: &str, wasm: &[u8]) -> serde_json::Value {
    serde_json::to_value(samples::manifest(id, version, wasm)).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn propose_review_approve_and_tools_follow_state() {
    let env = Env::new();
    let app = env.app();
    assert!(app.tools().is_empty(), "bez wtyczek silnik nie startuje");
    let wasm = component(&const_body(WORDS));
    let card = app
        .propose(manifest("licznik", "1.0.0", &wasm), &b64(&wasm))
        .await
        .unwrap();
    assert_eq!(card.state, PluginStateView::Proposed);
    assert_eq!(card.origin, PluginOrigin::User);
    assert_eq!(card.tools[0].name, "plugin_word_count");
    assert_eq!(card.limits.memory_mib, 16);
    let r2 = card.r2.clone().unwrap();
    assert_eq!(r2.key, "plugins.licznik.version");
    assert_eq!(r2.value, card.review_hash);
    assert!(app.tools().is_empty(), "propozycja nie daje narzędzi");

    let wrong = app
        .approve("licznik", "1.0.0", &"0".repeat(64))
        .await
        .unwrap_err();
    assert_eq!(wrong.code, ErrorCode::Forbidden);
    let installed = app
        .approve("licznik", "1.0.0", &card.review_hash)
        .await
        .unwrap();
    assert_eq!(installed.state, PluginStateView::Installed);
    assert!(installed.r2.is_none());
    let tools = app.tools();
    assert_eq!(tools.len(), 1);
    let out = tools[0].call(json!({"text": "ala ma kota"}), &ctx()).await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.data, json!({"words": 3}));
    assert!(out.untrusted.is_some(), "wynik wtyczki jest niezaufany");

    let off = app.disable("licznik").await.unwrap();
    assert_eq!(off.state, PluginStateView::Disabled);
    assert!(
        app.tools().is_empty(),
        "wyłączona — znika z rejestru od razu"
    );
    let on = app.enable("licznik", &card.review_hash).await.unwrap();
    assert_eq!(on.state, PluginStateView::Installed);
    assert_eq!(app.tools().len(), 1);

    let view = app.remove("licznik").await.unwrap();
    assert!(view.available && view.plugins.is_empty());
    assert!(app.tools().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn installed_plugins_survive_restart_and_reject_works() {
    let env = Env::new();
    let wasm = component(&const_body(WORDS));
    {
        let app = env.app();
        let card = app
            .propose(manifest("licznik", "1.0.0", &wasm), &b64(&wasm))
            .await
            .unwrap();
        app.approve("licznik", "1.0.0", &card.review_hash)
            .await
            .unwrap();
        let other = component(&const_body(br#"{"words":1}"#));
        let mut m = manifest("inna", "0.1.0", &other);
        m["tools"][0]["name"] = json!("inna_count");
        app.propose(m, &b64(&other)).await.unwrap();
        let rejected = app.reject("inna", "0.1.0").await.unwrap();
        assert_eq!(rejected.state, PluginStateView::Rejected);
    }
    let app = env.app();
    assert_eq!(
        app.tools().len(),
        1,
        "rekordy z dysku — narzędzia od startu"
    );
    let list = app.list().unwrap();
    assert_eq!(list.plugins.len(), 2);
    assert!(
        list.plugins
            .iter()
            .any(|p| p.state == PluginStateView::Rejected)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn trapped_plugin_degrades_health_for_the_diagnostician() {
    let env = Env::new();
    let app = env.app();
    let wasm = component(SPIN);
    let card = app
        .propose(manifest("petla", "1.0.0", &wasm), &b64(&wasm))
        .await
        .unwrap();
    app.approve("petla", "1.0.0", &card.review_hash)
        .await
        .unwrap();
    assert_eq!(app.health(), HealthStatus::Healthy);
    let out = app.tools()[0].call(json!({"text": "x"}), &ctx()).await;
    assert!(!out.is_ok());
    let problems = app.list().unwrap().problems;
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].kind, PluginProblemKind::Trapped);
    assert_eq!(problems[0].plugin, "petla");
    match app.health() {
        HealthStatus::Degraded(why) => assert!(why.contains("petla"), "{why}"),
        other => panic!("oczekiwano Degraded, jest {other:?}"),
    }
    let kinds: Vec<String> = env
        .bus
        .recorded()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert!(kinds.contains(&"plugin.trapped".to_owned()), "{kinds:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn inspect_and_bad_input_are_readable_errors() {
    let env = Env::new();
    let app = env.app();
    let good = component(&const_body(WORDS));
    let ok = app.inspect(&b64(&good)).await.unwrap();
    assert!(ok.ok && ok.error.is_none());
    assert_eq!(ok.wasm_sha256.len(), 64);
    assert_eq!(ok.bytes, good.len() as u64);
    let bad = app.inspect(&b64(b"\0asm-not-a-component")).await.unwrap();
    assert!(!bad.ok && bad.error.is_some());
    let e = app.inspect("%%%").await.unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    let e = app
        .propose(json!({"id": "x"}), &b64(&good))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    let mut m = manifest("zly", "1.0.0", &good);
    m["capabilities"] = json!([{"cap": "shell.exec", "scope": {"path": NOTES, "subtree": true}}]);
    let e = app.propose(m, &b64(&good)).await.unwrap_err();
    assert!(
        matches!(e.code, ErrorCode::Forbidden | ErrorCode::InvalidInput),
        "{e:?}"
    );
    let e = app.approve("licznik", "nie-semver", "x").await.unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    let e = app.disable("brak").await.unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[tokio::test]
async fn unavailable_without_broker_or_journal() {
    let app = app_plugins::PluginsApp::unavailable("brak Brokera");
    let view = app.list().unwrap();
    assert!(!view.available);
    assert!(view.unavailable_reason.unwrap().contains("brak Brokera"));
    let e = app.propose(json!({}), "").await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Unavailable);
    assert!(app.tools().is_empty());
    assert!(matches!(app.health(), HealthStatus::Unhealthy(_)));
}
