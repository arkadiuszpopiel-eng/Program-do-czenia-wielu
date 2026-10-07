//! Pomocniki testów v1: dostawca kierujący żądania do skryptów wg modelu (każda agentka ma
//! własny skrypt — równoległe przebiegi deterministyczne), narzędzie „wolne” z zapisem
//! przedziałów czasu (wykrywanie kolizji zasobów), obsada i persony.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_runtime_contract::{Crew, RunSpec, contract_tests::sample_spec};
use async_trait::async_trait;
use personas_contract::{Cast, PersonaId, RoleId, builtin_personas, builtin_roles};
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, ModelInfo, ModelProvider,
    ProviderCapabilities, ProviderError, ProviderHealth, ProviderId, ProviderStream, Usage,
};
use providers_fake::FakeProvider;
use tools_common_contract::{Tool, ToolCtx, ToolManifest, ToolOutcome};
use tools_fs_contract::{FsToolKind, manifest};

/// Dostawca: model → osobny `FakeProvider` (kolejka skryptów per agentka).
#[derive(Clone)]
pub struct Routed {
    id: ProviderId,
    by_model: BTreeMap<String, FakeProvider>,
}

impl Routed {
    pub fn new(models: &[&str]) -> Self {
        Self {
            id: ProviderId::new("routed"),
            by_model: models
                .iter()
                .map(|m| ((*m).to_owned(), FakeProvider::new(m)))
                .collect(),
        }
    }

    pub fn model(&self, m: &str) -> &FakeProvider {
        self.by_model
            .get(m)
            .unwrap_or_else(|| panic!("brak modelu {m}"))
    }
}

#[async_trait]
impl ModelProvider for Routed {
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn capabilities(&self) -> ProviderCapabilities {
        self.by_model.values().next().unwrap().capabilities()
    }
    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        let p = self.model(&request.model).clone();
        p.stream(request, cancel)
    }
    fn health(&self) -> ProviderHealth {
        ProviderHealth::healthy()
    }
    fn estimate_cost(&self, _request: &ChatRequest) -> Option<CostEstimate> {
        None
    }
    fn cost(&self, _model: &str, _usage: &Usage) -> Option<Cost> {
        None
    }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(Vec::new())
    }
}

/// Przedział wywołania narzędzia (ścieżka, agentka, start, koniec — czas wirtualny).
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub path: String,
    pub agent: String,
    pub start: tokio::time::Instant,
    pub end: tokio::time::Instant,
}

/// Narzędzie trwające `ms` (wirtualnie) z zapisem przedziałów.
pub struct Slow {
    manifest: ToolManifest,
    ms: u64,
    pub spans: Arc<Mutex<Vec<Span>>>,
    outcome: ToolOutcome,
}

impl Slow {
    pub fn new(kind: FsToolKind, ms: u64, spans: Arc<Mutex<Vec<Span>>>) -> Self {
        Self {
            manifest: manifest(kind),
            ms,
            spans,
            outcome: ToolOutcome::ok("wykonano", serde_json::json!({})),
        }
    }

    pub fn with_outcome(mut self, outcome: ToolOutcome) -> Self {
        self.outcome = outcome;
        self
    }
}

#[async_trait]
impl Tool for Slow {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        let start = tokio::time::Instant::now();
        tokio::select! {
            () = tokio::time::sleep(Duration::from_millis(self.ms)) => {}
            () = ctx.cancel.cancelled() => return ToolOutcome::cancelled("wolne narzędzie"),
        }
        let path = args
            .get("path")
            .and_then(|p| p.as_str())
            .unwrap_or_default()
            .to_owned();
        let agent = ctx
            .holder
            .agent
            .as_ref()
            .map(|a| a.as_str().to_owned())
            .unwrap_or_default();
        self.spans.lock().unwrap().push(Span {
            path,
            agent,
            start,
            end: tokio::time::Instant::now(),
        });
        self.outcome.clone()
    }
}

/// Czy dwa wywołania na tej samej ścieżce zachodziły na siebie w czasie.
pub fn collisions(spans: &[Span]) -> usize {
    let mut n = 0;
    for (i, a) in spans.iter().enumerate() {
        for b in &spans[i + 1..] {
            if a.path == b.path && a.start < b.end && b.start < a.end {
                n += 1;
            }
        }
    }
    n
}

/// Specyfikacja dla persony w roli (model = `m-<persona>`).
pub fn spec_for(persona: &str, role: &str, tools: &[&str]) -> RunSpec {
    let mut s = sample_spec(&format!("m-{persona}"), tools);
    let p = builtin_personas()
        .into_iter()
        .find(|x| x.id.as_str() == persona)
        .unwrap();
    s.agent = core_bus_contract::AgentId::new(persona);
    s.persona = p;
    s.roles = builtin_roles()
        .into_iter()
        .filter(|r| r.id.as_str() == role)
        .collect();
    s.goal = format!("Zadanie dla {persona}: uporządkuj /Users/ala/Documents");
    s
}

/// Obsada „Standard” z wbudowanymi personami i rolami; modele per rola.
pub fn standard_crew(models: &[(&str, &str)]) -> Crew {
    let assignments = [
        ("alfa", vec!["conductor", "speaker"]),
        ("beta", vec!["keeper", "writer"]),
        ("gama", vec!["researcher", "critic", "thinker"]),
        ("delta", vec!["operator", "coder"]),
    ]
    .into_iter()
    .map(|(p, roles)| {
        (
            PersonaId::new(p),
            roles.into_iter().map(RoleId::new).collect(),
        )
    })
    .collect();
    Crew {
        cast: Cast::new(None, false, assignments),
        personas: builtin_personas(),
        roles: builtin_roles(),
        models: models
            .iter()
            .map(|(r, m)| (RoleId::new(*r), (*m).to_owned()))
            .collect(),
    }
}

/// Tura z kilkoma wywołaniami narzędzi naraz.
pub fn multi_call(calls: &[(&str, &str, serde_json::Value)]) -> providers_fake::Script {
    use providers_contract::{ProviderEvent, StopReason, ToolArguments};
    use providers_fake::Step;
    let mut steps = vec![Step::Emit(ProviderEvent::Started {
        model: providers_fake::FAKE_MODEL.into(),
        response_id: None,
    })];
    for (index, (id, name, args)) in calls.iter().enumerate() {
        let index = u32::try_from(index).unwrap();
        steps.push(Step::Emit(ProviderEvent::ToolCallStart {
            index,
            id: (*id).into(),
            name: (*name).into(),
        }));
        steps.push(Step::Emit(ProviderEvent::ToolCallEnd {
            index,
            id: (*id).into(),
            arguments: ToolArguments::Parsed {
                value: args.clone(),
            },
        }));
    }
    steps.push(Step::Emit(ProviderEvent::stop(StopReason::ToolUse)));
    providers_fake::Script::new(steps)
}

/// Bramka schedulera w testach: dyrektywa per numer wywołania `boundary` (1-based), zapis
/// raportów kroków i chwil (wywołania numer → ostatni numer zdarzenia przebiegu).
pub struct TestGate {
    pub plan: Mutex<BTreeMap<u32, scheduler_contract::StepDirective>>,
    pub reports: Mutex<Vec<scheduler_contract::StepReport>>,
    pub probe: Mutex<Option<Box<dyn Fn() -> u64 + Send + Sync>>>,
    pub marks: Mutex<BTreeMap<u32, u64>>,
}

impl TestGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            plan: Mutex::new(BTreeMap::new()),
            reports: Mutex::new(Vec::new()),
            probe: Mutex::new(None),
            marks: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn at(&self, call: u32, d: scheduler_contract::StepDirective) {
        self.plan.lock().unwrap().insert(call, d);
    }

    pub fn calls(&self) -> u32 {
        u32::try_from(self.reports.lock().unwrap().len()).unwrap()
    }
}

impl scheduler_contract::StepGate for TestGate {
    fn boundary(
        &self,
        report: scheduler_contract::StepReport,
    ) -> scheduler_contract::StepDirective {
        let n = {
            let mut r = self.reports.lock().unwrap();
            r.push(report);
            u32::try_from(r.len()).unwrap()
        };
        if let Some(probe) = self.probe.lock().unwrap().as_ref() {
            self.marks.lock().unwrap().insert(n, probe());
        }
        self.plan
            .lock()
            .unwrap()
            .remove(&n)
            .unwrap_or(scheduler_contract::StepDirective::Continue { steering: vec![] })
    }

    fn spawn(
        &self,
        _tasks: Vec<scheduler_contract::TaskSpec>,
    ) -> Result<Vec<scheduler_contract::TaskId>, scheduler_contract::TaskError> {
        Ok(Vec::new())
    }
}

/// Wysłanie zadania z ładunkiem pętli agentki.
pub fn dispatch(
    task: &str,
    attempt: u32,
    spec: RunSpec,
    options: agent_runtime_contract::RunOptions,
    origin: scheduler_contract::TaskOrigin,
) -> scheduler_contract::Dispatch {
    let mut t = scheduler_contract::TaskSpec::new(
        task,
        "zadanie testowe",
        scheduler_contract::Assignee::Persona(PersonaId::new(spec.agent.as_str())),
        scheduler_contract::TaskClass::User,
        origin,
    );
    let agent = PersonaId::new(spec.agent.as_str());
    t.payload =
        serde_json::to_value(agent_runtime_contract::AgentTaskPayload { spec, options }).unwrap();
    scheduler_contract::Dispatch {
        dispatch: scheduler_contract::DispatchId(1),
        task: scheduler_contract::TaskId::new(task),
        attempt,
        agent: Some(agent),
        resume_from_step: 0,
        interrupted: false,
        spec: t,
        steering: Vec::new(),
        inputs: BTreeMap::new(),
    }
}

/// Wiadomość sterująca schedulera.
pub fn envelope(seq: u64, steer: scheduler_contract::Steer) -> scheduler_contract::SteerEnvelope {
    scheduler_contract::SteerEnvelope {
        seq,
        steer,
        sent_at_ms: 0,
        sent_at_step: 0,
    }
}
