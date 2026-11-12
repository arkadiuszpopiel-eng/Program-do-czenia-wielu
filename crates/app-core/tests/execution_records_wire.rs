//! Core i produkcyjny kodek lokalny: fakty gałęzi na przewodzie, bez prawdziwego modelu/HTTP.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
#[path = "support/history_seed.rs"]
mod history_seed;

use app_core::dto::{AgentDraft, SettingValue, WorkdirChoice};
use app_core::ports::{BrainChoice, BrainError, BrainPort, BrainRequest, HeadlessShell};
use app_core::{AppCore, AppPaths};
use async_trait::async_trait;
use common::{ScriptedProvider, ends, send, until};
use lib_openai_compat::{BuildOptions, WireCodec};
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, ModelInfo, ModelProvider,
    ProviderCapabilities, ProviderError, ProviderHealth, ProviderId, ProviderStream, Usage,
};
use providers_fake::{FAKE_MODEL, Script};
use providers_local_impl::{LlamaCodec, MAIN_MODEL, ModelEntry, builtin_models};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const MARK: &str = "[Zapis czynności aplikacji]";
const JARVIS: &str = "Tryb współpracy Jarvis";

struct CodecSpy {
    inner: Arc<ScriptedProvider>,
    entry: ModelEntry,
    bodies: Mutex<Vec<(bool, Value)>>,
}

// Budujemy format produkcyjnego adaptera, a odpowiedź pochodzi jawnie z atrapy.
// Nie uruchamiamy serwera, nie czytamy kont/kluczy, nie oceniamy jakości modelu.
#[async_trait]
impl ModelProvider for CodecSpy {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }
    fn stream(&self, mut request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        request.params.max_tokens = Some(96);
        request.params.temperature = Some(0.0);
        for tool_schema in [false, true] {
            let codec = LlamaCodec::new(&self.entry, 8192).with_tool_schema(tool_schema);
            let wire = codec.build(&request, BuildOptions::default()).unwrap();
            self.bodies.lock().unwrap().push((tool_schema, wire.body));
        }
        self.inner.stream(request, cancel)
    }
    fn health(&self) -> ProviderHealth {
        self.inner.health()
    }
    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        self.inner.estimate_cost(request)
    }
    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        self.inner.cost(model, usage)
    }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.inner.list_models().await
    }
}

struct WireBrain(Arc<CodecSpy>);

#[async_trait]
impl BrainPort for WireBrain {
    async fn choose(&self, _req: &BrainRequest) -> Result<BrainChoice, BrainError> {
        Ok(BrainChoice {
            provider: self.0.clone(),
            provider_id: "fake".into(),
            provider_name: "Atrapa odpowiedzi, rzeczywisty kodek".into(),
            account: None,
            model: FAKE_MODEL.into(),
            context_window: Some(8192),
            routed: false,
            route: None,
        })
    }
    fn keys_configured(&self) -> bool {
        true
    }
}

async fn create_luna(core: &AppCore) -> String {
    let draft: AgentDraft = serde_json::from_value(json!({
        "name": "Luna", "glyph": "L", "color": "color.agent.custom-2",
        "character": "Spokojna i konkretna.",
        "role": {"id": "czytelniczka", "name": "Czytelniczka", "description": "Czyta.",
                 "prompt": "Czytam pliki i krótko raportuję.", "model_policy": "conversation",
                 "tools": ["fs"], "read_only": true, "untrusted_isolated": false, "author": false},
        "limits": {"autonomy": "L1", "memory_scope": "agent", "retain_days": 30}
    }))
    .unwrap();
    let dry = core.builder_dry_run(draft.clone()).await.unwrap();
    assert!(dry.passed);
    core.builder_save(draft, dry.hash).await.unwrap().role
}

fn text(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        return s.into();
    }
    value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn check_body(body: &Value, name: &str, jarvis: bool, scenario: &str) {
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert!(messages[1..].iter().all(|m| m["role"] != "system"));
    let system = text(&messages[0]["content"]);
    assert!(system.contains(name), "tożsamość {name}");
    assert_eq!(system.contains(JARVIS), jarvis);
    assert_eq!(messages.last().unwrap()["role"], "user");
    assert!(text(&messages.last().unwrap()["content"]).contains(history_seed::QUERY));
    let data: Vec<_> = messages
        .iter()
        .filter_map(|m| {
            let content = text(&m["content"]);
            let (_, json) = content.split_once(MARK)?;
            assert_eq!(
                m["role"], "user",
                "fakty nie są instrukcją System lub akcją Assistant"
            );
            Some(
                serde_json::Deserializer::from_str(json.trim())
                    .into_iter::<Value>()
                    .next()
                    .unwrap()
                    .unwrap(),
            )
        })
        .collect();
    assert_eq!(data.len(), 1, "jedna nota, także po dopasowaniu okna");
    let facts = &data[0];
    assert_eq!(facts["source"], "application_execution_records");
    assert_eq!(facts["lookup_is_not_agent_action"], true);
    assert_eq!(
        facts["counts"]["recorded_done"],
        u64::from(scenario == "recorded_done")
    );
    assert_eq!(
        facts["counts"]["needs_confirmation"],
        u64::from(scenario == "needs_confirmation")
    );
    assert_eq!(facts["counts"]["unverified"], 0);
    assert!(!body.to_string().contains("sekret-testowy"));
    assert!(messages.iter().all(|m| {
        m["role"] != "tool"
            && m.get("tool_calls")
                .is_none_or(|v| v.is_null() || v.as_array().is_some_and(Vec::is_empty))
    }));
    assert_eq!(body["max_tokens"], 96);
}

// Opcjonalny zapis wyłącznie syntetycznych ciał, bez nagłówków/autoryzacji lub HTTP.
fn capture(mode: &str, agent: &str, scenario: &str, bodies: &[(bool, Value)]) {
    if let Some(root) = std::env::var_os("ALFA_WIRE_CAPTURE") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        for (schema, body) in bodies {
            let format = if *schema { "schema" } else { "native" };
            let path = root.join(format!("{mode}-{agent}-{scenario}-{format}.json"));
            std::fs::write(path, serde_json::to_vec_pretty(body).unwrap()).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn records_and_personas_survive_local_window_and_both_tool_formats() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(dir.path());
    let scripted = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let spy = Arc::new(CodecSpy {
        inner: scripted.clone(),
        entry: builtin_models()
            .unwrap()
            .into_iter()
            .find(|m| m.id == MAIN_MODEL)
            .unwrap(),
        bodies: Mutex::new(Vec::new()),
    });
    let mut opts = common::options(None, Arc::new(HeadlessShell::default()));
    let secrets = opts.secrets.clone().unwrap();
    opts.exec = Some(Arc::new(platform_fake::FakeExec::new()));
    opts.brain = Some(Arc::new(WireBrain(spy.clone())));
    let core = AppCore::build(paths.clone(), opts).await.unwrap();
    core.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let luna_role = create_luna(&core).await;
    let mut events = core.subscribe_events();
    for mode in ["standard", "jarvis"] {
        core.settings_set(
            "agents.interaction_mode".into(),
            SettingValue::Text(mode.into()),
        )
        .await
        .unwrap();
        for (agent, name) in [
            ("alfa", "Alfa"),
            ("beta", "Beta"),
            ("gama", "Gama"),
            ("delta", "Delta"),
            ("luna", "Luna"),
        ] {
            for scenario in ["empty_record", "recorded_done", "needs_confirmation"] {
                let sid = history_seed::seed(&core, &paths, secrets.clone(), agent, scenario).await;
                core.sessions_choose_workdir(sid.clone(), WorkdirChoice::Default)
                    .await
                    .unwrap();
                if agent == "luna" {
                    core.agents_set_roles(sid.clone(), agent.into(), vec![luna_role.clone()])
                        .await
                        .unwrap();
                }
                let before = spy.bodies.lock().unwrap().len();
                scripted.push(Script::text(FAKE_MODEL, &["Odpowiedź próby kodeka."]));
                let mut message = send(history_seed::QUERY, None);
                message.addressed_to = Some(agent.into());
                let turn = core
                    .turns_send(sid.clone(), message)
                    .await
                    .unwrap()
                    .assistant_turn_id
                    .unwrap();
                until(&mut events, ends(&turn)).await;
                let bodies = spy.bodies.lock().unwrap()[before..].to_vec();
                assert_eq!(
                    bodies.len(),
                    2,
                    "jedno żądanie, dwa kodeki: {agent}/{mode}/{scenario}"
                );
                for (_, body) in &bodies {
                    check_body(body, name, mode == "jarvis", scenario);
                }
                capture(mode, agent, scenario, &bodies);
                let turns = core.turns_list(sid).await.unwrap();
                let answer = turns.turns.iter().find(|t| t.id == turn).unwrap();
                assert_eq!(answer.text, "Odpowiedź próby kodeka.");
                assert!(
                    answer.tools.is_empty(),
                    "formatowanie nie jest akcją narzędzia"
                );
            }
        }
    }
    let bodies = spy.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 60);
    assert!(
        bodies
            .iter()
            .any(|(schema, b)| *schema && b.get("response_format").is_some())
    );
}
