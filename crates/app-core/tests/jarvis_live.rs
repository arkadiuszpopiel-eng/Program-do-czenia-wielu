//! Ręczna próba rozmowy wszystkich person na rzeczywistym modelu lokalnym, bez Tauri i mikrofonu.
//! Zmienne: ALFA_LIVE_SERVER, ALFA_LIVE_MODELS_DIR, ALFA_LIVE_MODEL, ALFA_LIVE_REPORT.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::{AlfaEvent, SessionTemplate, SettingValue, WorkdirChoice};
use app_core::ports::{BrainChoice, BrainError, BrainPort, BrainRequest, HeadlessShell};
use app_core::{AppCore, AppPaths};
use async_trait::async_trait;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, ModelInfo, ModelProvider,
    ProviderCapabilities, ProviderError, ProviderHealth, ProviderId, ProviderStream, Usage,
};
use providers_local_impl::{LocalConfig, LocalProvider, Sidecar, TokioLauncher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct LocalBrain {
    provider: Arc<dyn ModelProvider>,
    model: String,
    seen: Mutex<Vec<(String, ChatRequest)>>,
}

/// Jawny budżet tej próby integracji; nie zmienia ustawień produkcyjnego adaptera.
struct BoundedProvider(LocalProvider);

#[async_trait]
impl ModelProvider for BoundedProvider {
    fn id(&self) -> &ProviderId {
        self.0.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        self.0.capabilities()
    }
    fn stream(&self, mut request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        request.params.max_tokens = Some(96);
        request.params.temperature = Some(0.0);
        self.0.stream(request, cancel)
    }
    fn health(&self) -> ProviderHealth {
        self.0.health()
    }
    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        self.0.estimate_cost(request)
    }
    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        self.0.cost(model, usage)
    }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.0.list_models().await
    }
}

#[async_trait]
impl BrainPort for LocalBrain {
    async fn choose(&self, req: &BrainRequest) -> Result<BrainChoice, BrainError> {
        if let Some(chat) = &req.chat {
            self.seen
                .lock()
                .unwrap()
                .push((req.agent.clone(), chat.clone()));
        }
        Ok(BrainChoice {
            provider: self.provider.clone(),
            provider_id: "local".into(),
            provider_name: "llama.cpp (Cloud CPU)".into(),
            account: None,
            model: self.model.clone(),
            context_window: Some(8192),
            routed: false,
            route: None,
        })
    }
    fn keys_configured(&self) -> bool {
        false
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rzeczywisty model lokalny; ALFA_LIVE_* — raport, bez mikrofonu i Windows"]
async fn all_personas_on_local_model() {
    let server = std::env::var("ALFA_LIVE_SERVER").expect("ALFA_LIVE_SERVER");
    let models_dir = std::env::var("ALFA_LIVE_MODELS_DIR").expect("ALFA_LIVE_MODELS_DIR");
    let model = std::env::var("ALFA_LIVE_MODEL").expect("ALFA_LIVE_MODEL");
    let models = providers_local_impl::builtin_models().unwrap();
    assert!(models.iter().any(|m| m.id == model));
    let mut config = LocalConfig::new(models_dir, server);
    config.default_model.clone_from(&model);
    config.threads = Some(4);
    let sidecar = Sidecar::new(config, Arc::new(TokioLauncher::default()), None, None).unwrap();
    let brain = Arc::new(LocalBrain {
        provider: Arc::new(BoundedProvider(LocalProvider::new(models, sidecar.clone()))),
        model: model.clone(),
        seen: Mutex::new(Vec::new()),
    });
    let dir = tempfile::tempdir().unwrap();
    let mut opts = common::options(None, Arc::new(HeadlessShell::default()));
    opts.brain = Some(brain.clone());
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut events = core.subscribe_events();
    let mut results = Vec::new();
    for mode in ["standard", "jarvis"] {
        core.settings_set(
            "agents.interaction_mode".into(),
            SettingValue::Text(mode.into()),
        )
        .await
        .unwrap();
        // Pięć powtórzeń pozwala odróżnić pojedynczy udany wynik od stabilnej integracji.
        for repeat in 1..=5 {
            for agent in ["alfa", "beta", "gama", "delta"] {
                let session = core
                    .sessions_create(SessionTemplate::Empty)
                    .await
                    .unwrap()
                    .id;
                core.sessions_choose_workdir(session.clone(), WorkdirChoice::None)
                    .await
                    .unwrap();
                let mut message = common::send(
                    "Jednym krótkim zdaniem przedstaw się swoim imieniem i powiedz, że jesteś gotowa pomóc.",
                    None,
                );
                message.addressed_to = Some(agent.into());
                let started = Instant::now();
                let turn = core
                    .turns_send(session.clone(), message)
                    .await
                    .unwrap()
                    .assistant_turn_id
                    .unwrap();
                let finished = tokio::time::timeout(Duration::from_secs(120), async {
                    loop {
                        match events.recv().await {
                            Ok(batch) if batch.iter().any(|e| matches!(e, AlfaEvent::Stop { turn_id, .. } | AlfaEvent::Error { turn_id, .. } if turn_id == &turn)) => break,
                            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {},
                            Err(e) => panic!("strumień zdarzeń: {e}"),
                        }
                    }
                }).await.is_ok();
                if !finished {
                    core.turns_stop(session.clone()).await.unwrap();
                    sidecar.stop("koniec czasu próby Jarvis").await;
                }
                assert!(finished, "{agent}/{mode}/{repeat}: przekroczone 120 s");
                let saved = core.turns_list(session).await.unwrap();
                let answer = saved.turns.iter().find(|t| t.id == turn).unwrap();
                let elapsed_ms = started.elapsed().as_millis();
                results.push(serde_json::json!({"agent":agent,"mode":mode,"repeat":repeat,"elapsed_ms":elapsed_ms,
                    "text":answer.text,"error":answer.error,"response_author":answer.author,
                    "truncated":answer.truncated,"usage":answer.usage,
                    "mentions_own_name": answer.text.to_lowercase().contains(agent)}));
                if let Ok(path) = std::env::var("ALFA_LIVE_REPORT") {
                    let report = serde_json::json!({"model":model,"platform":"Linux Cloud CPU; headless core, no microphone", "max_tokens":96, "temperature":0.0, "results":results});
                    std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
                }
                assert!(answer.error.is_none(), "{agent}/{mode}: {:?}", answer.error);
                assert_eq!(answer.author, agent, "autor zapisanej odpowiedzi");
                assert!(
                    !answer.text.trim().is_empty(),
                    "{agent}/{mode}: pusta odpowiedź"
                );
                let seen = brain.seen.lock().unwrap();
                let (who, request) = seen.last().unwrap();
                assert_eq!(who, agent);
                assert_eq!(
                    request
                        .system
                        .as_deref()
                        .unwrap_or_default()
                        .contains("Tryb współpracy Jarvis"),
                    mode == "jarvis"
                );
                eprintln!("{agent}/{mode}/{repeat}: {elapsed_ms} ms");
            }
        }
    }
    sidecar.stop("ukończona próba Jarvis").await;
}
