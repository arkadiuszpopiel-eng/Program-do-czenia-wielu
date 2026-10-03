//! Stan wspólny runtime: zależności, rejestr przebiegów (także podprzebiegów), start pętli
//! z hakami (granica kroku schedulera, posiadaczka dzierżaw) i ponowne podjęcie przebiegu
//! oddanego schedulerowi na tym samym uchwycie (ciągłość dziennika i kolejki sterowania).
//! Skażenie sesji (przegląd #2, P2-07): każdy start pętli dziedziczy taint sesji z rejestru
//! [`SessionTaint`], a zdarzenie `Tainted` dowolnego przebiegu skaża sesję (monotonicznie).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use agent_runtime_contract::{
    Checkpoint, CheckpointStore, MemorySessionTaint, RunError, RunId, RunOutcome, SessionTaint,
    TaintReset, TaintResetError,
};
use core_bus_contract::{AgentId, EventBus, SessionId};
use providers_contract::ModelProvider;
use risk_classifier_contract::AutonomyLevel;
use safety_broker_contract::{Broker, TaintSource};
use scheduler_contract::{Holder as LockHolder, SteerEnvelope, StepGate};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tools_common_contract::Tool;

use crate::RuntimeConfig;
use crate::engine::Engine;
use crate::handle::RunHandle;
use crate::registry::ToolRegistry;

/// Poziom autonomii agentki w sesji (sufit delegacji: potomek ≤ rodzic).
pub trait AutonomyOracle: Send + Sync {
    /// Poziom obowiązujący agentkę w sesji.
    fn level(&self, session: &SessionId, agent: &AgentId) -> AutonomyLevel;
}

/// Adapter: poziomy z Brokera (`Broker::autonomy`).
pub struct BrokerAutonomy(pub Arc<dyn Broker>);

impl AutonomyOracle for BrokerAutonomy {
    fn level(&self, session: &SessionId, agent: &AgentId) -> AutonomyLevel {
        self.0.autonomy(session, Some(agent))
    }
}

/// Skażenie sesji z Brokera (`SessionSecurity`, monotoniczne do końca sesji) połączone
/// z rejestrem w pamięci procesu — źródło prawdy dla runtime (P2-07). Reset czyści tylko część
/// lokalną; dopóki Broker trzyma taint sesji, kolejne przebiegi nadal startują skażone.
pub struct BrokerSessionTaint {
    broker: Arc<dyn Broker>,
    local: MemorySessionTaint,
}

impl BrokerSessionTaint {
    /// Rejestr nad Brokerem.
    pub fn new(broker: Arc<dyn Broker>) -> Self {
        Self {
            broker,
            local: MemorySessionTaint::default(),
        }
    }
}

impl SessionTaint for BrokerSessionTaint {
    fn taint(&self, session: &SessionId) -> Option<TaintSource> {
        self.local.taint(session).or_else(|| {
            let security = self.broker.session_security(session);
            security
                .taint_sources
                .first()
                .cloned()
                .or_else(|| security.tainted.then_some(TaintSource::File))
        })
    }

    fn mark(&self, session: &SessionId, source: &TaintSource) {
        self.local.mark(session, source);
    }

    fn reset(&self, session: &SessionId, confirmation: &TaintReset) -> Result<(), TaintResetError> {
        self.local.reset(session, confirmation)
    }
}

/// Zależności v1 (opcjonalne; bez nich runtime działa jak v0 + delegacja/Krytyczka z opcji).
#[derive(Clone, Default)]
pub struct RuntimeExt {
    /// Zasoby wyłączne (`scheduler-lite` albo pełny scheduler): dzierżawy plików i ekranu na
    /// czas wywołań narzędzi zmieniających stan — równoległe agentki bez kolizji.
    pub locks: Option<Arc<dyn scheduler_contract::SchedulerLite>>,
    /// Poziomy autonomii (delegacja odrzucana, gdy wykonawczyni ma wyższy poziom niż zlecająca).
    pub autonomy: Option<Arc<dyn AutonomyOracle>>,
}

/// Haki jednego startu pętli (nie trafiają do checkpointu).
#[derive(Clone, Default)]
pub(crate) struct Hooks {
    /// Granica kroku schedulera (`StepGate::boundary` po każdym kroku atomowym).
    pub(crate) gate: Option<Arc<dyn StepGate>>,
    /// Posiadaczka dzierżaw (zadanie schedulera: ta sama co dzierżawy zadania).
    pub(crate) lease_holder: Option<LockHolder>,
    /// Kurs USD→PLN × 10⁴ (koszt kroku w raporcie dla schedulera).
    pub(crate) usd_pln_e4: u64,
    /// Sterowanie sprzed startu (`Dispatch::steering`) — trafia do pierwszego kroku.
    pub(crate) initial_steering: Vec<SteerEnvelope>,
}

/// Jak skończył się start pętli.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Exit {
    /// Przebieg zakończony.
    Finished(RunOutcome),
    /// Przebieg zatrzymany dyrektywą schedulera (`StepDirective::Stop`).
    Stopped(RunOutcome),
    /// Oddany schedulerowi (pauza/wywłaszczenie) — checkpoint zapisany, wznowienie później.
    Yielded,
}

pub(crate) struct Shared {
    pub(crate) provider: Arc<dyn ModelProvider>,
    pub(crate) tools: Vec<Arc<dyn Tool>>,
    pub(crate) store: Arc<dyn CheckpointStore>,
    pub(crate) config: RuntimeConfig,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
    pub(crate) ext: RuntimeExt,
    /// Skażenie sesji (P2-07): domyślnie rejestr w pamięci procesu.
    pub(crate) taint: Arc<dyn SessionTaint>,
    runs: Mutex<BTreeMap<RunId, Arc<RunHandle>>>,
}

impl Shared {
    pub(crate) fn new(
        provider: Arc<dyn ModelProvider>,
        tools: Vec<Arc<dyn Tool>>,
        store: Arc<dyn CheckpointStore>,
        bus: Option<Arc<dyn EventBus>>,
        config: RuntimeConfig,
    ) -> Self {
        Self {
            provider,
            tools,
            store,
            config,
            bus,
            ext: RuntimeExt::default(),
            taint: Arc::new(MemorySessionTaint::default()),
            runs: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn with_ext(mut self, ext: RuntimeExt) -> Self {
        self.ext = ext;
        self
    }

    pub(crate) fn with_taint(mut self, taint: Arc<dyn SessionTaint>) -> Self {
        self.taint = taint;
        self
    }

    fn runs(&self) -> MutexGuard<'_, BTreeMap<RunId, Arc<RunHandle>>> {
        self.runs.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub(crate) fn handle(&self, run: &RunId) -> Result<Arc<RunHandle>, RunError> {
        self.runs()
            .get(run)
            .cloned()
            .ok_or_else(|| RunError::UnknownRun(run.clone()))
    }

    pub(crate) fn find(&self, run: &RunId) -> Option<Arc<RunHandle>> {
        self.runs().get(run).cloned()
    }

    /// Startuje (albo podejmuje) pętlę przebiegu. Uchwyt przebiegu oddanego schedulerowi jest
    /// używany ponownie; `cancel` = token podprzebiegu (anulowanie rodzica obejmuje potomka).
    pub(crate) fn launch(
        self: &Arc<Self>,
        mut cp: Checkpoint,
        hooks: Hooks,
        cancel: Option<CancellationToken>,
    ) -> Result<(Arc<RunHandle>, JoinHandle<Exit>), RunError> {
        // Taint sesji (wcześniejsze tury, inne przebiegi) — przebieg startuje skażony (P2-07).
        if cp.taint_source.is_none()
            && let Some(source) = self.taint.taint(&cp.spec.session)
        {
            cp.tainted = true;
            cp.taint_source = Some(source);
        }
        let registry = ToolRegistry::for_run(
            &self.tools,
            &cp.spec,
            &cp.options,
            self.config.max_delegation_depth,
        )
        .map_err(RunError::InvalidSpec)?;
        let run = cp.run.clone();
        let handle = {
            let mut runs = self.runs();
            let handle = match runs.get(&run) {
                Some(h) if h.is_running() => return Err(RunError::AlreadyRunning(run)),
                Some(h) if h.is_active() => h.clone(),
                _ => Arc::new(
                    RunHandle::new(
                        run.clone(),
                        cp.spec.session.clone(),
                        cp.spec.agent.clone(),
                        self.bus.clone(),
                        cancel.unwrap_or_default(),
                    )
                    .with_taint(self.taint.clone()),
                ),
            };
            handle.set_running(true);
            runs.insert(run.clone(), handle.clone());
            handle
        };
        let engine = Engine::new(self.clone(), handle.clone(), cp, registry, hooks);
        Ok((handle, tokio::spawn(engine.run())))
    }
}
