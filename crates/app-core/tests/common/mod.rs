//! Wspólne narzędzia testów integracyjnych `app-core`: rdzeń na katalogu tymczasowym, atrapy
//! (`providers-fake`, `device-profile-fake`, sekrety w pamięci), zbieranie paczek zdarzeń.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_core::dto::{AlfaEvent, SendOptions, StopReason};
use app_core::ports::{BrainChoice, BrainError, BrainPort, BrainRequest, HeadlessShell};
use app_core::{AppCore, AppOptions, AppPaths, EventBatch, MemorySecretStore};
use async_trait::async_trait;
use device_profile_fake::FakeDeviceProfile;
use providers_contract::{
    CancellationToken, ChatRequest, ContentBlock, ModelInfo, ModelProvider, Pricing,
    ProviderCapabilities, ProviderError, ProviderHealth, ProviderId, ProviderStream, Role, Usage,
};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;

/// Cennik atrapy (wysoki, żeby koszt w groszach był > 0).
pub fn pricing() -> Pricing {
    Pricing {
        input_per_mtok_usd: 3_000.0,
        output_per_mtok_usd: 15_000.0,
        cache_read_per_mtok_usd: None,
        cache_write_per_mtok_usd: None,
    }
}

fn fresh() -> FakeProvider {
    FakeProvider::new("fake").with_pricing(FAKE_MODEL, pricing())
}

/// Ostatnia wiadomość użytkownika z żądania.
pub fn last_user_text(request: &ChatRequest) -> String {
    request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == Role::User)
        .map(|m| {
            m.content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// Dostawca testowy: skrypt z kolejki albo echo ostatniej wiadomości (słowami, z odstępem).
pub struct ScriptedProvider {
    base: FakeProvider,
    queue: Mutex<VecDeque<Script>>,
    seen: Mutex<Vec<ChatRequest>>,
    gap: Duration,
}

impl ScriptedProvider {
    pub fn new(gap: Duration) -> Self {
        Self {
            base: fresh(),
            queue: Mutex::new(VecDeque::new()),
            seen: Mutex::new(Vec::new()),
            gap,
        }
    }

    pub fn push(&self, script: Script) {
        self.queue.lock().unwrap().push_back(script);
    }

    pub fn requests(&self) -> Vec<ChatRequest> {
        self.seen.lock().unwrap().clone()
    }
}

#[async_trait]
impl ModelProvider for ScriptedProvider {
    fn id(&self) -> &ProviderId {
        self.base.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        self.base.capabilities()
    }
    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        self.seen.lock().unwrap().push(request.clone());
        let script = self.queue.lock().unwrap().pop_front().unwrap_or_else(|| {
            let echo = format!("Echo: {}", last_user_text(&request));
            let words: Vec<String> = echo.split_inclusive(' ').map(str::to_owned).collect();
            Script::chunks(FAKE_MODEL, &words, self.gap)
        });
        fresh().with_default_script(script).stream(request, cancel)
    }
    fn health(&self) -> ProviderHealth {
        self.base.health()
    }
    fn estimate_cost(&self, request: &ChatRequest) -> Option<providers_contract::CostEstimate> {
        self.base.estimate_cost(request)
    }
    fn cost(&self, model: &str, usage: &Usage) -> Option<providers_contract::Cost> {
        self.base.cost(model, usage)
    }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.base.list_models().await
    }
}

/// „Mózg" testowy: zawsze ten sam dostawca atrapy.
pub struct TestBrain {
    pub provider: Arc<ScriptedProvider>,
}

#[async_trait]
impl BrainPort for TestBrain {
    async fn choose(&self, _request: &BrainRequest) -> Result<BrainChoice, BrainError> {
        Ok(BrainChoice {
            provider: self.provider.clone(),
            provider_id: "fake".into(),
            provider_name: "Atrapa".into(),
            account: None,
            model: FAKE_MODEL.into(),
            context_window: Some(32_000),
            routed: false,
            route: None,
        })
    }
    fn keys_configured(&self) -> bool {
        true
    }
}

/// Rdzeń testowy.
pub struct Harness {
    pub core: AppCore,
    pub provider: Arc<ScriptedProvider>,
    pub shell: Arc<HeadlessShell>,
    pub rx: Receiver<EventBatch>,
    pub dir: tempfile::TempDir,
}

pub fn options(provider: Option<Arc<ScriptedProvider>>, shell: Arc<HeadlessShell>) -> AppOptions {
    AppOptions {
        frame: Duration::from_millis(2),
        undo_window: Some(Duration::from_millis(300)),
        fetch_fx: false,
        file_logs: false,
        secrets: Some(Arc::new(MemorySecretStore::default())),
        device: Some(Arc::new(FakeDeviceProfile::desktop())),
        brain: provider.map(|p| Arc::new(TestBrain { provider: p }) as Arc<dyn BrainPort>),
        shell: Some(shell),
        // Wirtualne audio (deterministyczne na każdym systemie); TTS — brak sidecarów.
        audio: Some(Arc::new(voice_audio_fake::FakeAudio::new())),
        ..AppOptions::default()
    }
}

/// Rdzeń z dostawcą atrapy (echo co `gap`).
pub async fn harness_with(gap: Duration, with_brain: bool) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(gap));
    let shell = Arc::new(HeadlessShell::default());
    let brain = with_brain.then(|| provider.clone());
    let core = AppCore::build(AppPaths::under(dir.path()), options(brain, shell.clone()))
        .await
        .unwrap();
    let rx = core.subscribe_events();
    Harness {
        core,
        provider,
        shell,
        rx,
        dir,
    }
}

pub async fn harness() -> Harness {
    harness_with(Duration::from_millis(1), true).await
}

/// Zbiera zdarzenia aż do spełnienia warunku (limit 20 s).
pub async fn until(
    rx: &mut Receiver<EventBatch>,
    mut done: impl FnMut(&AlfaEvent) -> bool,
) -> Vec<AlfaEvent> {
    let mut seen = Vec::new();
    let found = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            match rx.recv().await {
                Ok(batch) => {
                    // Cała paczka trafia do wyniku (kolejne zdarzenia paczki nie giną).
                    let mut matched = false;
                    for e in batch.iter() {
                        seen.push(e.clone());
                        matched |= done(e);
                    }
                    if matched {
                        return true;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return false,
            }
        }
    })
    .await;
    assert!(
        matches!(found, Ok(true)),
        "nie doczekano się zdarzenia; widziane: {seen:#?}"
    );
    seen
}

/// Czy zdarzenie kończy turę `turn`.
pub fn ends(turn: &str) -> impl FnMut(&AlfaEvent) -> bool + '_ {
    move |e| match e {
        AlfaEvent::Stop { turn_id, .. } | AlfaEvent::Error { turn_id, .. } => turn_id == turn,
        _ => false,
    }
}

pub fn stop_reason(events: &[AlfaEvent], turn: &str) -> Option<StopReason> {
    events.iter().find_map(|e| match e {
        AlfaEvent::Stop {
            turn_id, reason, ..
        } if turn_id == turn => Some(*reason),
        _ => None,
    })
}

pub fn send(text: &str, parent: Option<String>) -> SendOptions {
    SendOptions {
        parent_id: parent,
        text: text.into(),
        addressed_to: None,
        profile: None,
    }
}

/// Próg czasu: ścisły przy `ALFA_PERF_BUDGETS=1`, inaczej ×10 (crates/README.md).
pub fn budget(ms: u64) -> Duration {
    let strict = std::env::var("ALFA_PERF_BUDGETS").is_ok_and(|v| v == "1");
    Duration::from_millis(if strict { ms } else { ms * 10 })
}
