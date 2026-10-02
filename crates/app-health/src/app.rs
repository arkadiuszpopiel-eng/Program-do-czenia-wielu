//! `HealthApp`: budowa i start Diagnosty, Ulepszacza i evali, komendy strony „Zdrowie systemu"
//! (raport, skan, zgoda/odrzucenie/cofnięcie naprawy, propozycje Ulepszacza, wyniki evali) i most
//! zdarzeń `diagnostician.*`/`improver.*`/`evals.gate.*` → `HealthChanged`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use app_api::dto::{
    AlfaEvent, EvalSuiteView, EvalVerdictView, EvalsView, HealthOverall, HealthView, ImproverView,
    ModuleHealth, iso,
};
use app_api::ports::BrainPort;
use app_api::{AppError, EventHub};
use core_bus_contract::{BusItem, EventBus, EventFilter};
use core_config_contract::ConfigStore;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleId, Registry};
use diagnostician_contract::{DiagError, Diagnostician, RepairId, UserConsent};
use diagnostician_impl::DiagnosticianService;
use evals_contract::{CandidateRunner, GateVerdict, SuiteCatalog, SuiteId};
use evals_impl::{DirCatalog, EvalsModule, HoldoutGate};
use futures_util::StreamExt;
use improver_contract::{
    Improver, ImproverError, ImproverPolicy, MetricsSnapshot, ProposalId, RunConditions,
    UserApproval,
};
use improver_impl::ImproverService;

use crate::diag::{DiagDeps, open as open_diag};
use crate::improve::{
    LocalProposer, NoReplayRunner, SURFACE, UiDigestVerifier, evals_root, open_evals,
};
use crate::view;

mod ops;

/// Odstęp sprawdzania warunków cyklu Ulepszacza w bezczynności.
pub const IDLE_CYCLE_EVERY: Duration = Duration::from_secs(15 * 60);
/// Ile ostatnich werdyktów bramki pokazuje strona.
const MAX_VERDICTS: usize = 20;

/// Zależności „Zdrowia systemu".
pub struct HealthDeps {
    /// Dane lokalne (`%LOCALAPPDATA%\Alfa`).
    pub local: PathBuf,
    /// Konfiguracja (naprawy Diagnosty, wdrożenia Ulepszacza — z historią i origin modułu).
    pub config: Arc<dyn ConfigStore>,
    /// Rejestr modułów (stan, restart, sonda).
    pub registry: Arc<dyn Registry>,
    /// Magistrala (sygnały Diagnosty, zdarzenia modułów).
    pub bus: Arc<dyn EventBus>,
    /// Zdarzenia UI.
    pub events: EventHub,
    /// Korzenie Jądra (Diagnosta nie dotyka ich bez Brokera).
    pub kernel_roots: Vec<PathBuf>,
    /// Katalog zestawów (`None` — obok programu albo w danych Alfy).
    pub evals_root: Option<PathBuf>,
    /// Uruchamianie wariantu w piaskownicy (`None` — replay niedostępny: bramka odmawia).
    pub runner: Option<Arc<dyn CandidateRunner>>,
    /// Odstęp skanu Diagnosty (`None` — domyślny, 30 s).
    pub scan_every_ms: Option<u64>,
}

type EvalParts = (Arc<EvalsModule>, Arc<DirCatalog>, Arc<HoldoutGate>);

/// Strona „Zdrowie systemu".
pub struct HealthApp {
    diag: Result<Arc<DiagnosticianService>, String>,
    improver: Result<Arc<ImproverService>, String>,
    evals: Result<EvalParts, String>,
    proposer: Arc<LocalProposer>,
    registry: Arc<dyn Registry>,
    events: EventHub,
    verdicts: Mutex<VecDeque<EvalVerdictView>>,
    last_cycle: Mutex<Option<String>>,
    idle_cycle: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for HealthApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthApp")
            .field("diagnostician", &self.diag.is_ok())
            .field("improver", &self.improver.is_ok())
            .field("evals", &self.evals.is_ok())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

async fn start<M: Module>(mut m: M, id: &str, bus: &Arc<dyn EventBus>) -> Result<Arc<M>, String> {
    let id = ModuleId::new(id).map_err(|e| e.to_string())?;
    m.start(ModuleContext::new(id, bus.clone()))
        .await
        .map_err(|e| e.to_string())?;
    Ok(Arc::new(m))
}

fn diag_error(e: DiagError) -> AppError {
    match e {
        DiagError::Unknown(_) => AppError::not_found(format!("Diagnosta: {e}")),
        DiagError::KernelAreaRequiresBroker(_) => AppError::forbidden(format!(
            "Diagnosta: {e}. Okno Brokera (Broker-UI) nie jest uruchomione w tym trybie."
        )),
        _ => AppError::invalid(format!("Diagnosta: {e}")),
    }
}

fn improver_error(e: ImproverError) -> AppError {
    AppError::invalid(format!("Ulepszacz: {e}"))
}

impl HealthApp {
    /// Buduje i uruchamia moduły (błąd jednego = moduł niezdrowy, komendy „niedostępne").
    pub async fn open(d: HealthDeps) -> Arc<Self> {
        let diag = async {
            let service = open_diag(&DiagDeps {
                local: d.local.clone(),
                config: d.config.clone(),
                registry: d.registry.clone(),
                kernel_roots: d.kernel_roots.clone(),
            })
            .await?;
            let service = match d.scan_every_ms {
                Some(ms) => service.with_scan_interval(ms),
                None => service,
            };
            start(service, "diagnostician", &d.bus).await
        }
        .await;
        let runner = d.runner.unwrap_or_else(|| Arc::new(NoReplayRunner));
        let root = d.evals_root.unwrap_or_else(|| evals_root(&d.local));
        let holdout = d.local.join("evals").join("holdout");
        let evals = async {
            let (catalog, gate) = open_evals(&root, &holdout, runner)?;
            let module = EvalsModule::new(catalog.clone()).map_err(|e| e.to_string())?;
            let module = start(module, "evals", &d.bus).await?;
            Ok::<EvalParts, String>((module, catalog, gate))
        }
        .await;
        let proposer = Arc::new(LocalProposer::default());
        let improver = async {
            let gate = evals.as_ref().map_err(Clone::clone)?.2.clone();
            let state = d.local.join("improver");
            std::fs::create_dir_all(&state).map_err(|e| format!("{}: {e}", state.display()))?;
            let service = ImproverService::with_proposers(
                d.config.clone(),
                gate,
                Arc::new(UiDigestVerifier),
                ImproverPolicy::default(),
                Arc::new(evals_contract::SystemClock),
                Some(state.join("proposals.json")),
                vec![proposer.clone()],
            )
            .map_err(|e| e.to_string())?;
            start(service, "improver", &d.bus).await
        }
        .await;
        Arc::new(Self {
            diag,
            improver,
            evals,
            proposer,
            registry: d.registry,
            events: d.events,
            verdicts: Mutex::new(VecDeque::new()),
            last_cycle: Mutex::new(None),
            idle_cycle: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Model lokalny dla propozycji Ulepszacza (po złożeniu portów).
    pub fn bind_brain(&self, brain: Arc<dyn BrainPort>) {
        self.proposer.bind(brain);
    }

    /// Zdrowie modułu dla rejestru.
    pub fn health(&self, id: &str) -> HealthStatus {
        let of = |r: Result<HealthStatus, &String>| {
            r.unwrap_or_else(|e| HealthStatus::Unhealthy(e.clone()))
        };
        match id {
            "diagnostician" => of(self.diag.as_ref().map(|m| m.health())),
            "improver" => of(self.improver.as_ref().map(|m| m.health())),
            "evals" => of(self.evals.as_ref().map(|(m, _, _)| m.health())),
            _ => HealthStatus::NotStarted,
        }
    }

    fn diag(&self) -> Result<&Arc<DiagnosticianService>, AppError> {
        self.diag
            .as_ref()
            .map_err(|_| AppError::unavailable("Diagnosta", "diagnostician"))
    }

    fn improver(&self) -> Result<&Arc<ImproverService>, AppError> {
        self.improver
            .as_ref()
            .map_err(|_| AppError::unavailable("Ulepszacz", "improver"))
    }

    /// Diagnosta (testy, wstrzykiwanie sygnałów przez kompozycję).
    pub fn diagnostician(&self) -> Option<Arc<DiagnosticianService>> {
        self.diag.as_ref().ok().cloned()
    }

    /// `health_report`.
    pub async fn report(&self) -> Result<HealthView, AppError> {
        let mut modules = Vec::new();
        for s in self.registry.list().await {
            let health = self.registry.health(&s.id).await.ok();
            modules.push(view::module(&s, health));
        }
        let unhealthy = modules.iter().any(|m| m.health == ModuleHealth::Unhealthy);
        let (overall, safe_mode, parts, problems) = match &self.diag {
            Ok(d) => {
                let r = d.report();
                let problems = d.journal_problems().to_vec();
                (
                    view::overall(r.overall),
                    r.safe_mode.clone(),
                    Some(view::report_parts(&r)),
                    problems,
                )
            }
            Err(e) => (
                HealthOverall::Degraded,
                None,
                None,
                vec![format!("Diagnosta: {e}")],
            ),
        };
        let overall = if overall == HealthOverall::Ok && unhealthy {
            HealthOverall::Degraded
        } else {
            overall
        };
        let parts = parts.unwrap_or(view::ReportParts {
            incidents: Vec::new(),
            repaired: Vec::new(),
            needs_human: Vec::new(),
            pending: Vec::new(),
        });
        Ok(HealthView {
            overall,
            safe_mode,
            generated_at: iso(chrono::Utc::now()),
            modules,
            incidents: parts.incidents,
            repaired: parts.repaired,
            needs_human: parts.needs_human,
            pending: parts.pending,
            problems,
        })
    }

    /// `health_scan`: skan teraz.
    pub async fn scan(&self) -> Result<HealthView, AppError> {
        self.diag()?.scan().await;
        self.report().await
    }

    /// `health_approve`: zgoda użytkownika (Jądro — wyłącznie Broker-UI).
    pub async fn approve(&self, id: u64) -> Result<HealthView, AppError> {
        let consent = UserConsent {
            surface: SURFACE.into(),
        };
        self.diag()?
            .approve(RepairId(id), consent)
            .await
            .map_err(diag_error)?;
        self.report().await
    }

    /// `health_reject`.
    pub async fn reject(&self, id: u64) -> Result<HealthView, AppError> {
        self.diag()?
            .reject(RepairId(id))
            .await
            .map_err(diag_error)?;
        self.report().await
    }

    /// `health_undo`: cofnięcie naprawy (operacje odwrotne, porównaj-i-zamień).
    pub async fn undo(&self, id: u64) -> Result<HealthView, AppError> {
        self.diag()?.undo(RepairId(id)).await.map_err(diag_error)?;
        self.report().await
    }

    async fn announce(&self) {
        let (overall, pending) = match &self.diag {
            Ok(d) => {
                let r = d.report();
                (view::overall(r.overall), r.pending.len())
            }
            Err(_) => (HealthOverall::Degraded, 0),
        };
        let awaiting = self
            .improver
            .as_ref()
            .map(|i| {
                i.proposals()
                    .iter()
                    .filter(|p| p.stage == improver_contract::Stage::AwaitingApproval)
                    .count()
            })
            .unwrap_or(0);
        self.events.emit(AlfaEvent::HealthChanged {
            overall,
            pending: u32::try_from(pending + awaiting).unwrap_or(u32::MAX),
        });
    }

    /// Most zdarzeń modułów → `HealthChanged` (i werdykty bramki do listy).
    pub async fn spawn_bridge(self: &Arc<Self>, bus: Arc<dyn EventBus>) {
        let mut streams = Vec::new();
        for prefix in ["diagnostician.", "improver.", "evals.gate."] {
            if let Ok(s) = bus.subscribe(EventFilter::prefix(prefix)).await {
                streams.push(s);
            }
        }
        let mut all = futures_util::stream::select_all(streams);
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            while let Some(item) = all.next().await {
                let BusItem::Event(event) = item else {
                    continue;
                };
                let Some(me) = me.upgrade() else {
                    return;
                };
                if event.kind.as_str().starts_with("evals.gate.")
                    && let Ok(v) = serde_json::from_value::<GateVerdict>(event.payload.clone())
                {
                    let at = u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0);
                    let mut list = lock(&me.verdicts);
                    list.push_front(view::verdict(&v, at));
                    list.truncate(MAX_VERDICTS);
                }
                me.announce().await;
            }
        });
    }

    /// Cykl Ulepszacza w bezczynności (co [`IDLE_CYCLE_EVERY`]): tylko gdy użytkownik bezczynny,
    /// nie na baterii i nie w grze — warunki z portu sygnałów systemu.
    pub fn spawn_idle_cycle(
        self: &Arc<Self>,
        conditions: Arc<dyn Fn() -> RunConditions + Send + Sync>,
        every: Duration,
    ) {
        self.idle_cycle
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(every).await;
                let Some(me) = me.upgrade() else {
                    return;
                };
                let c = conditions();
                if c.user_idle && !c.on_battery && !c.game_mode {
                    let _ = me.run_cycle(c).await;
                }
            }
        });
    }
}
