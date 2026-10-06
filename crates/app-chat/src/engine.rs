//! Stan silnika czatu: zależności (moduły złożone przez `app-core`), aktywne generacje i przebiegi
//! agentek per sesja, okna kontekstu modeli, kroki cofalne poza dziennikiem (schowek, zmienne)
//! należące do sesji oraz wiązanie z rdzeniem (`ChatHost`: ogłoszenie sesji, łączność, koszty —
//! rdzeń trzymany słabo, bez cyklu).

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;

use app_agents::{AgentTools, Launch, RunHandle, TicketLog};
use app_api::dto::{self, AlfaEvent, TurnError};
use app_api::events::EventHub;
use app_api::ids::UndoKind;
use app_api::ports::{BrainPort, BrokerPort};
use app_store::AppStore;
use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_config_contract::{ConfigKey, ConfigStore, Scope};
use cost_meter_contract::CostMeter;
use personas_contract::Personas;
use providers_contract::CancellationToken;
use sessions_contract::{SessionId, TurnId};
use sessions_impl::SqliteSessions;
use tokio::sync::watch;

/// Narzędzia agentek i rejestr próśb o zatwierdzenie.
#[derive(Clone)]
pub struct AgentStack {
    /// Narzędzia (wszystkie przebiegi).
    pub tools: Arc<AgentTools>,
    /// Fakty próśb o zatwierdzenie (karta w wątku).
    pub tickets: Arc<TicketLog>,
    /// Start v1: zasoby wyłączne (scheduler), autonomia (Broker), obsada, umiejętności.
    pub launch: Launch,
}

/// Aktywna generacja odpowiedzi w sesji (najwyżej jedna na sesję).
#[derive(Clone)]
pub struct GenHandle {
    /// Zarezerwowany identyfikator tury agentki.
    pub turn: TurnId,
    /// Agentka.
    pub agent: String,
    /// Anulowanie (Esc / STOP / nowa wiadomość).
    pub cancel: CancellationToken,
    /// `true` po zapisie tury.
    pub done: watch::Receiver<bool>,
    /// Bieżąca postać tury (dla `turns_list` w trakcie strumienia).
    pub live: Arc<Mutex<dto::Turn>>,
}

impl GenHandle {
    /// Czeka na zakończenie (z limitem czasu).
    pub async fn wait(&self, limit: Duration) {
        let mut done = self.done.clone();
        let _ = tokio::time::timeout(limit, done.wait_for(|d| *d)).await;
    }
}

/// Sterowanie trwającym przebiegiem (steering, anulowanie, stan „czeka").
#[derive(Clone)]
pub struct RunCtl {
    /// Przebieg.
    pub handle: Arc<RunHandle>,
    /// Agentka.
    pub agent: String,
    /// Czy czeka na zatwierdzenie.
    pub(crate) waiting: Arc<std::sync::atomic::AtomicBool>,
}

impl RunCtl {
    /// Czy agentka czeka na zatwierdzenie.
    pub fn waiting(&self) -> bool {
        self.waiting.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Rdzeń widziany przez silnik (implementuje `app-core`).
#[async_trait]
pub trait ChatHost: Send + Sync {
    /// `SessionUpdated` z bieżącą postacią sesji.
    async fn announce_session(&self, session: &SessionId);
    /// Wynik generacji → stan łączności (offline / 429; `answered` = model odpowiedział).
    async fn connectivity(&self, error: Option<&TurnError>, answered: bool);
    /// `CostsChanged` dla sesji po zapisie kosztu tury.
    async fn costs_changed(&self, session: &SessionId);
}

/// Moduły i porty, z których korzysta silnik (składa je `app-core`).
pub struct ChatDeps {
    /// Historia sesji (append-only) i katalog.
    pub sessions: Arc<SqliteSessions>,
    /// Fakty tur, oś czasu, Replay.
    pub store: Arc<AppStore>,
    /// Obsada i prompty agentek.
    pub personas: Arc<dyn Personas>,
    /// Pamięć F7 (zestaw roboczy w prompcie).
    pub memory: Arc<app_memory::MemoryApp>,
    /// Wybór modelu (Router / dostawca).
    pub brain: Arc<dyn BrainPort>,
    /// Koszty i budżet.
    pub costs: Arc<dyn CostMeter>,
    /// Narzędzia agentek (`None` — Broker albo dziennik cofania niepodłączony).
    pub agents: Option<AgentStack>,
    /// Broker (okno zatwierdzeń).
    pub broker: Arc<dyn BrokerPort>,
    /// Broker Jądra dla narzędzi (`None` — niepodłączony: agentki bez narzędzi) — skażenie sesji.
    pub kernel: Option<Arc<dyn safety_broker_contract::Broker>>,
    /// Zadania (delegacja do mostów CLI).
    pub tasks: Arc<app_tasks::TasksApp>,
    /// Magistrala (symptomy dostawców, zdarzenia `agent.*`).
    pub bus: Arc<dyn EventBus>,
    /// Zdarzenia UI.
    pub events: EventHub,
    /// Załączniki tur (projekcja dla modelu).
    pub files: Arc<app_files::FilesApp>,
    /// Konfiguracja (ustawienia agentek).
    pub config: Arc<dyn ConfigStore>,
    /// Limit czasu zatwierdzenia narzucony przez opcje aplikacji (testy).
    pub approval_timeout: Option<Duration>,
}

/// Ulotny stan silnika.
#[derive(Default)]
pub(crate) struct ChatState {
    pub gens: HashMap<SessionId, GenHandle>,
    /// Trwające przebiegi agentek (sesja → sterowanie).
    pub runs: HashMap<SessionId, RunCtl>,
    pub context_window: HashMap<SessionId, u64>,
    /// Kroki cofalne poza dziennikiem (schowek, zmienne użytkownika) — token należy do sesji,
    /// której przebieg go utworzył.
    pub owned_undo: HashMap<SessionId, BTreeSet<(UndoKind, u64)>>,
}

pub(crate) struct Engine {
    deps: ChatDeps,
    state: Mutex<ChatState>,
    host: OnceLock<Arc<dyn ChatHost>>,
}

impl std::ops::Deref for Engine {
    type Target = ChatDeps;

    fn deref(&self) -> &ChatDeps {
        &self.deps
    }
}

/// Silnik tury czatu (tani do klonowania).
#[derive(Clone)]
pub struct ChatEngine {
    pub(crate) inner: Arc<Engine>,
}

impl ChatEngine {
    /// Silnik nad złożonymi modułami (rdzeń wiąże się później: `bind_host`).
    pub fn new(deps: ChatDeps) -> Self {
        Self {
            inner: Arc::new(Engine {
                deps,
                state: Mutex::default(),
                host: OnceLock::new(),
            }),
        }
    }

    /// Wiąże rdzeń (raz; kolejne wywołania są ignorowane).
    pub fn bind_host(&self, host: Arc<dyn ChatHost>) {
        let _ = self.inner.host.set(host);
    }

    pub(crate) fn rt(&self) -> MutexGuard<'_, ChatState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn emit(&self, event: AlfaEvent) {
        self.inner.events.emit(event);
    }

    pub(crate) async fn announce_session(&self, session: &SessionId) {
        if let Some(host) = self.inner.host.get() {
            host.announce_session(session).await;
        }
    }

    pub(crate) fn host(&self) -> Option<&Arc<dyn ChatHost>> {
        self.inner.host.get()
    }

    /// Aktywna generacja w sesji.
    pub fn generation(&self, session: &SessionId) -> Option<GenHandle> {
        self.rt().gens.get(session).cloned()
    }

    /// Anuluje aktywną generację sesji i czeka na zapis jej tury.
    pub async fn finalize_generation(&self, session: &SessionId) {
        if let Some(handle) = self.generation(session) {
            handle.cancel.cancel();
            handle.wait(Duration::from_secs(10)).await;
        }
    }

    /// STOP WSZYSTKIEGO: anuluje generacje i przebiegi agentek we wszystkich sesjach (od razu);
    /// zwraca uchwyty generacji, na których zapis można poczekać.
    pub fn cancel_all(&self) -> Vec<GenHandle> {
        let handles: Vec<GenHandle> = self.rt().gens.values().cloned().collect();
        for h in &handles {
            h.cancel.cancel();
        }
        let runs: Vec<_> = self.rt().runs.values().map(|r| r.handle.clone()).collect();
        for run in &runs {
            run.cancel();
        }
        handles
    }

    /// Czy trwa jakikolwiek przebieg agentki (blokada restartu po aktualizacji).
    pub fn has_runs(&self) -> bool {
        !self.rt().runs.is_empty()
    }

    /// Trwający przebieg agentki w sesji (steering).
    pub fn run_handle(&self, session: &SessionId) -> Option<Arc<RunHandle>> {
        self.rt().runs.get(session).map(|r| r.handle.clone())
    }

    /// Okno kontekstu modelu ostatniej odpowiedzi w sesji.
    pub fn context_window(&self, session: &SessionId) -> Option<u64> {
        self.rt().context_window.get(session).copied()
    }

    /// Czy krok cofania (`kind`, `id`) spoza dziennika należy do sesji.
    pub fn owns_undo(&self, session: &SessionId, kind: UndoKind, id: u64) -> bool {
        self.rt()
            .owned_undo
            .get(session)
            .is_some_and(|ids| ids.contains(&(kind, id)))
    }

    /// Zapomina cofnięty krok.
    pub fn release_undo(&self, session: &SessionId, kind: UndoKind, id: u64) {
        if let Some(ids) = self.rt().owned_undo.get_mut(session) {
            ids.remove(&(kind, id));
        }
    }

    /// Wartość konfiguracji (globalnie).
    pub(crate) async fn config_value(&self, key: &str) -> Option<serde_json::Value> {
        let key = ConfigKey::new(key).ok()?;
        self.inner
            .config
            .get(&key, &Scope::Global)
            .await
            .ok()
            .flatten()
    }

    /// Wartość ustawienia jako bool (z domyślną).
    pub(crate) async fn config_bool(&self, key: &str, default: bool) -> bool {
        self.config_value(key)
            .await
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }
}
