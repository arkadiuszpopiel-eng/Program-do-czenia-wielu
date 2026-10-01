//! Wspólne narzędzia testów Routera: atrapy dostawców, rejestr zgodności, bramka budżetu.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::NaiveDate;
use compliance_contract::{
    Jurisdiction, PrivacyTag, ProviderApiStatus, ProviderPolicyInput, Registry, RouteTags,
};
use compliance_fake::FakeCompliance;
use cost_meter_contract::BudgetDecision;
use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, EmbeddingRequest, EmbeddingResponse,
    HealthState, Message, ModelCapabilities, ModelInfo, ModelProvider, Pricing,
    ProviderCapabilities, ProviderError, ProviderEvent, ProviderHealth, ProviderId, ProviderStream,
    Usage,
};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use router_contract::{BudgetGate, Candidate};

pub fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()
}

pub fn pricing() -> Pricing {
    Pricing {
        input_per_mtok_usd: 4.0,
        output_per_mtok_usd: 20.0,
        cache_read_per_mtok_usd: None,
        cache_write_per_mtok_usd: None,
    }
}

/// Atrapa z odpowiedzią domyślną „`<id>` odpowiada" i cennikiem.
pub fn fake(id: &str) -> FakeProvider {
    FakeProvider::new(id)
        .with_pricing(FAKE_MODEL, pricing())
        .with_default_script(Script::text(FAKE_MODEL, &[id, " odpowiada"]))
}

pub fn cand(p: &str) -> Candidate {
    Candidate::new(p, FAKE_MODEL)
}

pub fn req(text: &str) -> ChatRequest {
    ChatRequest::new("auto", vec![Message::user_text(text)])
}

/// Wpis katalogu z tagami (zielona trasa API).
pub fn catalog_entry(
    provider: &str,
    tags: &[PrivacyTag],
    jurisdiction: &str,
) -> ProviderPolicyInput {
    ProviderPolicyInput {
        provider: provider.into(),
        tags: RouteTags {
            privacy: tags.iter().copied().collect::<BTreeSet<_>>(),
            jurisdiction: jurisdiction.parse::<Jurisdiction>().unwrap(),
        },
        api_status: ProviderApiStatus::Green,
    }
}

pub fn compliance(entries: Vec<ProviderPolicyInput>) -> FakeCompliance {
    let registry = Registry {
        schema_version: 1,
        max_age_days: 30,
        generated_at: day(),
        notes: String::new(),
        routes: vec![],
        providers: vec![],
    };
    FakeCompliance::new(registry, entries, day())
}

pub async fn collect(p: &dyn ModelProvider, r: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(r, CancellationToken::new()).collect().await
}

pub fn text(events: &[ProviderEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Bramka budżetu ze sterowanym werdyktem per dostawca.
#[derive(Default)]
pub struct ScriptedGate {
    pub verdicts: Mutex<Vec<(ProviderId, BudgetDecision)>>,
    pub asked: Mutex<Vec<(ProviderId, u64, bool)>>,
}

impl BudgetGate for ScriptedGate {
    fn check(&self, provider: &ProviderId, micro_usd: u64, background: bool) -> BudgetDecision {
        self.asked
            .lock()
            .unwrap()
            .push((provider.clone(), micro_usd, background));
        self.verdicts
            .lock()
            .unwrap()
            .iter()
            .find(|(p, _)| p == provider)
            .map_or(BudgetDecision::Allow, |(_, d)| d.clone())
    }
}

/// Dostawca z sterowanym stanem zdrowia (np. `Unconfigured` = brak klucza) nad atrapą.
pub struct Switchable {
    pub inner: FakeProvider,
    pub state: Mutex<Option<ProviderHealth>>,
}

impl Switchable {
    pub fn new(inner: FakeProvider, health: Option<ProviderHealth>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            state: Mutex::new(health),
        })
    }

    pub fn set(&self, health: Option<ProviderHealth>) {
        *self.state.lock().unwrap() = health;
    }
}

pub fn unconfigured() -> Option<ProviderHealth> {
    Some(ProviderHealth {
        state: HealthState::Unconfigured,
        ..ProviderHealth::healthy()
    })
}

#[async_trait]
impl ModelProvider for Switchable {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }
    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        self.inner.stream(request, cancel)
    }
    fn health(&self) -> ProviderHealth {
        self.state
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.inner.health())
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
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        self.inner.embed(request).await
    }
}

pub fn caps(vision: bool, tools: bool) -> ModelCapabilities {
    ModelCapabilities {
        vision,
        tools,
        ..ModelCapabilities::default()
    }
}

/// Budżet czasowy (crates/README.md „Testy budżetów czasowych"): ściśle przy `ALFA_PERF_BUDGETS=1`,
/// na współdzielonym CI próg bezpieczeństwa ×10.
pub fn budget(strict: std::time::Duration) -> std::time::Duration {
    if std::env::var_os("ALFA_PERF_BUDGETS").is_some() {
        strict
    } else {
        strict * 10
    }
}
