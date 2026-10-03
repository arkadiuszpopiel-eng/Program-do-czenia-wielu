//! Cykl życia (wspólny test kontraktowy), licznik słów w rejestrze narzędzi i w pętli
//! model → narzędzie → model na atrapach (`providers-fake`, `safety-broker-fake`), magazyn
//! w katalogu po „restarcie”, moduł w rejestrze.

use std::sync::Arc;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleId};
use futures_util::StreamExt;
use plugin_runtime_contract::{
    MemHost, PluginSource, Plugins, RUN_CAPABILITY, UNTRUSTED_SOURCE, contract_tests, samples,
};
use plugin_runtime_impl::{DirPluginStore, PluginDeps, PluginRuntime, RuntimeConfig};
use providers_contract::{
    CancellationToken, ChatRequest, ContentBlock, Message, ModelProvider, ProviderId, Role,
    ToolResult, ToolResultPart, TurnAccumulator,
};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use safety_broker_fake::FakeBroker;
use serde_json::json;

use crate::common::*;

#[tokio::test(flavor = "multi_thread")]
async fn contract_lifecycle_on_impl() {
    let h = harness();
    contract_tests::lifecycle(&h.runtime, &word_count_wasm).await;
    let names = h.event_names();
    for e in [
        "plugin.proposed",
        "plugin.installed",
        "plugin.superseded",
        "plugin.disabled",
        "plugin.enabled",
        "plugin.rejected",
        "plugin.removed",
    ] {
        assert!(names.contains(&e.to_owned()), "brak {e}");
    }
    assert_eq!(h.store.wasm_count(), 0, "usunięcie kasuje moduły");
}

#[tokio::test(flavor = "multi_thread")]
async fn word_count_plugin_registered_for_role_with_plugin_group() {
    let h = harness();
    let wasm = word_count_wasm(0);
    h.install(samples::manifest("licznik", "1.0.0", &wasm), wasm)
        .await;
    let catalog = h.runtime.tool_catalog();
    let m = &catalog[0];
    assert_eq!(m.name, "plugin_word_count");
    assert!(m.validate().is_ok());
    assert_eq!(m.capabilities, vec![RUN_CAPABILITY.to_owned()]);
    assert_eq!(m.untrusted_output, Some(UNTRUSTED_SOURCE));
    assert!(
        m.allowed_for(&["plugin".into()], true),
        "rola z grupą `plugin`"
    );
    assert!(
        m.allowed_for(&["plugin.licznik".into()], false),
        "rola z grupą tej wtyczki"
    );
    assert!(
        !m.allowed_for(&["fs".into(), "shell".into()], false),
        "rola bez grupy wtyczek"
    );
    let out = h
        .tool("plugin_word_count")
        .call(json!({"text": "Ala ma kota, a kot ma Alę."}), &ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.data, json!({"words": 7}));
    assert_eq!(out.untrusted, Some(UNTRUSTED_SOURCE));
}

/// Pętla agentki w miniaturze (jak `agent-runtime`): specyfikacje narzędzi z rejestru →
/// model woła narzędzie → wynik wraca jako `tool_result` → model odpowiada.
#[tokio::test(flavor = "multi_thread")]
async fn model_calls_plugin_tool_through_registry() {
    let h = harness();
    let wasm = word_count_wasm(0);
    h.install(samples::manifest("licznik", "1.0.0", &wasm), wasm)
        .await;
    let registry = h.runtime.tools();
    let fake = FakeProvider::new("fake");
    // ASCII: atrapa dzieli argumenty w połowie bajtów (bez granic znaków).
    let args = json!({"text": "Ala ma kota"});
    fake.push_script(Script::tool_call(
        FAKE_MODEL,
        "call-1",
        "plugin_word_count",
        &args,
    ));
    fake.push_script(Script::text(FAKE_MODEL, &["Tekst ma 3 słowa."]));

    let mut messages = vec![Message::user_text("Ile słów ma „Ala ma kota”?")];
    let mut final_text = String::new();
    for _ in 0..3 {
        let mut req = ChatRequest::new(FAKE_MODEL, messages.clone());
        req.tools = registry.iter().map(|t| t.manifest().to_spec()).collect();
        let events: Vec<_> = fake.stream(req, CancellationToken::new()).collect().await;
        let mut acc = TurnAccumulator::new(ProviderId::new("fake"));
        for e in &events {
            acc.push(e);
        }
        let turn = acc.finish();
        let uses: Vec<_> = turn.message.tool_uses().cloned().collect();
        messages.push(turn.message.clone());
        if uses.is_empty() {
            final_text = turn.message.visible_text();
            break;
        }
        let mut results = Vec::new();
        for u in uses {
            let tool = registry
                .iter()
                .find(|t| t.manifest().name == u.name)
                .unwrap();
            let out = tool.call(u.input.clone(), &ctx()).await;
            assert_eq!(out.data, json!({"words": 3}), "{}", out.text);
            results.push(ContentBlock::ToolResult(ToolResult {
                tool_use_id: u.id.clone(),
                is_error: !out.is_ok(),
                content: vec![ToolResultPart::Text { text: out.text }],
            }));
        }
        messages.push(Message::new(Role::User, results));
    }
    assert_eq!(final_text, "Tekst ma 3 słowa.");
    let second = &fake.requests()[1];
    assert!(
        serde_json::to_string(&second.messages)
            .unwrap()
            .contains(r#"{\"words\":3}"#)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dir_store_survives_restart_and_remove_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let deps = |store: Arc<DirPluginStore>| PluginDeps {
        broker: Arc::new(FakeBroker::new().unwrap()),
        host: Arc::new(MemHost::default()),
        store,
        bus: None,
        config: RuntimeConfig::default(),
    };
    let store = Arc::new(DirPluginStore::open(dir.path()).unwrap());
    let rt = PluginRuntime::new(deps(store.clone())).unwrap();
    let wasm = word_count_wasm(0);
    let r = rt
        .propose(
            samples::manifest("licznik", "1.0.0", &wasm),
            wasm.clone(),
            PluginSource::User,
        )
        .await
        .unwrap();
    rt.approve(
        &r.manifest.id,
        &r.manifest.version,
        plugin_runtime_contract::PluginApproval::ui(&r.review_hash),
    )
    .await
    .unwrap();
    let file = dir.path().join("wasm").join(format!("{}.wasm", sha(&wasm)));
    assert!(file.exists());
    drop(rt);

    let rt = PluginRuntime::new(deps(store.clone())).unwrap();
    let tool = rt.tools().into_iter().next().unwrap();
    let out = tool.call(json!({"text": "raz dwa"}), &ctx()).await;
    assert_eq!(out.data, json!({"words": 2}), "{}", out.text);
    rt.remove(&r.manifest.id).await.unwrap();
    assert!(!file.exists() && rt.list().is_empty());
    assert!(plugin_runtime_contract::PluginStore::get_wasm(&*store, "../../etc/passwd").is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn module_registry_lifecycle() {
    let mut h = harness();
    let manifest = plugin_runtime_impl::module_manifest().unwrap();
    assert_eq!(manifest.id.as_str(), "plugin-runtime");
    assert_eq!(h.runtime.health(), HealthStatus::NotStarted);
    let ctx = ModuleContext::new(
        ModuleId::new("plugin-runtime").unwrap(),
        Arc::new(FakeBus::default()),
    );
    h.runtime.start(ctx.clone()).await.unwrap();
    assert!(h.runtime.start(ctx).await.is_err());
    assert_eq!(h.runtime.health(), HealthStatus::Healthy);
    h.runtime.stop().await.unwrap();
    assert!(h.runtime.stop().await.is_err());
    assert_eq!(h.runtime.manifest().id.as_str(), "plugin-runtime");
    assert!(h.runtime.tick().as_millis() > 0);
}
