//! `AppCore`: stan współdzielony kompozycji i wspólne pomocniki komend (blokady sesji,
//! aktywne generacje, stan systemu, projekcja sesji do DTO).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use accounts_hub_impl::AccountsHubService;
use artifacts_impl::SqliteArtifacts;
use chrono::{DateTime, Utc};
use compliance_impl::ComplianceService;
use core_bus_contract::EventBus;
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_impl::FileConfigStore;
use core_registry_impl::ModuleRegistry;
use cost_meter_impl::CostMeterService;
use device_profile_contract::DeviceProfile as DeviceProfileService;
use personas_impl::PersonasModule;
use providers_contract::CancellationToken;
use search_impl::SqliteSearch;
use sessions_contract::{SessionCatalog, SessionId, TurnId};
use sessions_impl::SqliteSessions;
use tokio::sync::{OwnedMutexGuard, watch};

use crate::dto::{self, AlfaEvent, AutonomyLevel, ModelProfile, SessionSummary};
use crate::error::AppError;
use crate::events::{EventBatch, EventHub};
use crate::options::AppPaths;
use crate::ports::{BrainPort, BrokerPort, ShellPort, TransferPort, VoicePort};
use crate::settings::{SettingsCatalog, keys};
use crate::store::AppStore;

/// Aktywna generacja odpowiedzi w sesji (najwyżej jedna na sesję).
#[derive(Clone)]
pub(crate) struct GenHandle {
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

/// Ulotny stan rdzenia.
#[derive(Default)]
pub(crate) struct Runtime {
    pub online: bool,
    pub rate_limit: Option<(String, DateTime<Utc>)>,
    pub queued: BTreeMap<SessionId, Vec<TurnId>>,
    pub gens: HashMap<SessionId, GenHandle>,
    pub trash: HashMap<String, (SessionId, tokio::task::AbortHandle)>,
    pub context_window: HashMap<SessionId, u64>,
    /// Trwające pobierania modeli lokalnych (model → anulowanie).
    pub downloads: HashMap<String, CancellationToken>,
    /// Trwające przebiegi agentek (sesja → sterowanie).
    pub runs: HashMap<SessionId, crate::chat::agent::RunCtl>,
    /// Zapisy schowka cofalne w sesji (token schowka należy do sesji, która go utworzyła).
    pub clip_undo: HashMap<SessionId, std::collections::BTreeSet<u64>>,
}

/// Współdzielony stan kompozycji.
pub(crate) struct Inner {
    pub paths: AppPaths,
    pub app_version: String,
    /// Po jakim czasie bez awarii start jest zdrowy (`updater::mark_good`).
    pub healthy_after: Duration,
    pub undo_window: Option<Duration>,
    pub approval_timeout: Option<Duration>,
    pub bus: Arc<dyn EventBus>,
    pub registry: Arc<ModuleRegistry>,
    pub config: Arc<FileConfigStore>,
    pub machine: MachineId,
    pub sessions: Arc<SqliteSessions>,
    pub search: Arc<SqliteSearch>,
    /// Pamięć F7 (Inspektor, narzędzia agentek, kontekst czatu, Strażniczka).
    pub memory: Arc<app_memory::MemoryApp>,
    pub artifacts: Arc<SqliteArtifacts>,
    pub hub: Arc<AccountsHubService>,
    pub costs: Arc<CostMeterService>,
    /// Utrzymuje moduł zgodności (używa go `accounts-hub`; tu — dla zdrowia w rejestrze).
    pub _compliance: Arc<ComplianceService>,
    pub device: Arc<dyn DeviceProfileService>,
    pub personas: Arc<PersonasModule>,
    /// Zadania (DAG, scheduler z tablicą blokad głosu), wyzwalacze i reguły Marszałka.
    pub tasks: Arc<app_tasks::TasksApp>,
    /// Mosty CLI (karty zgodności, delegacja, logowanie w terminalu) i serwer MCP na żądanie.
    pub bridges: Arc<app_bridges::BridgesApp>,
    pub brain: Arc<dyn BrainPort>,
    /// Moduły podpięte po F1 (rezydencja, model lokalny, Router, Broker, transfer, głos,
    /// aktualizacje) — trzymane przez cały czas życia rdzenia.
    pub extra: crate::parts::Extra,
    pub transfer: Arc<dyn TransferPort>,
    pub voice: Arc<dyn VoicePort>,
    pub broker: Arc<dyn BrokerPort>,
    /// Narzędzia agentek (`None` — Broker albo dziennik cofania niepodłączony: bez narzędzi).
    pub agents: Option<crate::parts::AgentStack>,
    pub shell: Arc<dyn ShellPort>,
    pub events: EventHub,
    pub store: AppStore,
    pub settings: SettingsCatalog,
    pub runtime: Mutex<Runtime>,
    pub locks: Mutex<HashMap<SessionId, Arc<tokio::sync::Mutex<()>>>>,
}

/// Rdzeń aplikacji: komendy IPC jako metody `async` + strumień zdarzeń. Tani do klonowania.
#[derive(Clone)]
pub struct AppCore {
    pub(crate) inner: Arc<Inner>,
}

impl AppCore {
    /// Subskrypcja paczek zdarzeń `alfa://events` (powłoka emituje każdą paczkę do okien).
    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<EventBatch> {
        self.inner.events.subscribe()
    }

    /// Ścieżki aplikacji.
    pub fn paths(&self) -> &AppPaths {
        &self.inner.paths
    }

    /// Wersja aplikacji.
    pub fn app_version(&self) -> &str {
        &self.inner.app_version
    }

    /// Dziennik cofania `fs.*` (`undo-journal`) dla narzędzi plików (agent-runtime) — tokeny kart
    /// „Cofnij" w UI: `ids::undo_dto(sesja, krok)`. `None` = moduł niepodłączony.
    pub fn undo_journal(&self) -> Option<Arc<dyn undo_journal_contract::UndoJournal>> {
        self.inner
            .extra
            .undo
            .clone()
            .map(|u| u as Arc<dyn undo_journal_contract::UndoJournal>)
    }

    pub(crate) fn rt(&self) -> MutexGuard<'_, Runtime> {
        self.inner
            .runtime
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn emit(&self, event: AlfaEvent) {
        self.inner.events.emit(event);
    }

    /// Zdarzenie od powłoki (np. `Toast` o konflikcie skrótu globalnego) — trafia do paczki.
    pub fn emit_event(&self, event: AlfaEvent) {
        self.emit(event);
    }

    /// Komunikat błędu w języku interfejsu (`ui.locale`) — odrzucenie `invoke` w UI.
    pub async fn error_text(&self, error: &AppError) -> String {
        let locale = self.config_str(crate::settings::keys::LOCALE).await;
        error.localized(locale.as_deref().unwrap_or("pl"))
    }

    /// Blokada zapisu historii sesji (dopisywanie tur jest szeregowane per sesja).
    pub(crate) async fn lock_session(&self, session: &SessionId) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self
                .inner
                .locks
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            locks.entry(session.clone()).or_default().clone()
        };
        lock.lock_owned().await
    }

    /// Aktywna generacja w sesji.
    pub(crate) fn generation(&self, session: &SessionId) -> Option<GenHandle> {
        self.rt().gens.get(session).cloned()
    }

    /// Anuluje aktywną generację sesji i czeka na zapis jej tury.
    pub(crate) async fn finalize_generation(&self, session: &SessionId) {
        if let Some(handle) = self.generation(session) {
            handle.cancel.cancel();
            handle.wait(Duration::from_secs(10)).await;
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

    /// Zapis konfiguracji przez użytkownika (`machine` = nakładka per maszyna).
    pub(crate) async fn config_set(
        &self,
        key: &str,
        value: Option<serde_json::Value>,
        machine: bool,
    ) -> Result<(), AppError> {
        let key = ConfigKey::new(key)?;
        let layer = if machine {
            ConfigLayer::Machine(self.inner.machine.clone())
        } else {
            ConfigLayer::Shared
        };
        self.inner
            .config
            .set(&key, value, &Scope::Global, &layer, Origin::User)
            .await
            .map_err(AppError::from)
    }

    /// Wartość ustawienia jako tekst.
    pub(crate) async fn config_str(&self, key: &str) -> Option<String> {
        self.config_value(key)
            .await
            .and_then(|v| v.as_str().map(str::to_owned))
    }

    /// Wartość ustawienia jako bool (z domyślną).
    pub(crate) async fn config_bool(&self, key: &str, default: bool) -> bool {
        self.config_value(key)
            .await
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }

    /// Ustawienie dla powłoki (np. `general.close_to_tray`, `general.destroy_webview_after`).
    pub async fn setting(&self, key: &str) -> Option<dto::SettingValue> {
        self.config_value(key)
            .await
            .as_ref()
            .and_then(dto::SettingValue::from_json)
    }

    /// Poziom autonomii globalny.
    pub(crate) async fn autonomy(&self) -> AutonomyLevel {
        if let Some(view) = self.inner.broker.levels(None).await {
            return view.global;
        }
        let raw = self.config_str(keys::AUTONOMY).await;
        raw.and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok())
            .unwrap_or_default()
    }

    /// Poziom autonomii obowiązujący w sesji (z Brokera; bez niego — globalny z konfiguracji).
    pub(crate) async fn session_autonomy(&self, session: &SessionId) -> AutonomyLevel {
        match self.inner.broker.levels(Some(session)).await {
            Some(view) => view.session.unwrap_or(view.global),
            None => self.autonomy().await,
        }
    }

    /// `SessionUpdated` dla wszystkich sesji (np. po zmianie poziomu globalnego).
    pub(crate) async fn announce_all_sessions(&self) {
        let all = self
            .inner
            .sessions
            .list_sessions(&sessions_contract::SessionQuery::default())
            .unwrap_or_default();
        for s in &all {
            let session = self.session_dto(s).await;
            self.emit(AlfaEvent::SessionUpdated { session });
        }
    }

    /// Profil modelu domyślny (bez kluczy — lokalny).
    pub(crate) async fn default_profile(&self) -> ModelProfile {
        if !self.inner.brain.keys_configured() {
            return ModelProfile::Local;
        }
        self.config_str(keys::DEFAULT_PROFILE)
            .await
            .and_then(|s| ModelProfile::parse(&s))
            .unwrap_or_default()
    }

    /// Projekcja pozycji katalogu sesji do DTO.
    pub(crate) async fn session_dto(
        &self,
        s: &sessions_contract::SessionSummary,
    ) -> SessionSummary {
        let working = s.active || self.generation(&s.meta.id).is_some();
        let profile = match ModelProfile::parse(&s.meta.model_policy) {
            Some(p) => p,
            None => self.default_profile().await,
        };
        SessionSummary {
            id: s.meta.id.to_string(),
            title: s.meta.title.clone(),
            project: s.meta.project.as_ref().map(|p| dto::ProjectRef {
                id: p.0.clone(),
                name: p.0.clone(),
            }),
            pinned: s.meta.pinned,
            archived: s.meta.archived,
            working,
            unread: s.unread > 0,
            updated_at: dto::iso(s.activity_at().max(s.meta.updated_at)),
            tags: s.meta.tags.clone(),
            autonomy: self.session_autonomy(&s.meta.id).await,
            profile,
        }
    }

    /// Bieżąca pozycja listy sesji.
    pub(crate) async fn session_summary(&self, id: &SessionId) -> Result<SessionSummary, AppError> {
        let all = self
            .inner
            .sessions
            .list_sessions(&sessions_contract::SessionQuery {
                include_archived: true,
                ..Default::default()
            })
            .map_err(AppError::from)?;
        let found = all
            .iter()
            .find(|s| &s.meta.id == id)
            .ok_or_else(|| AppError::not_found(format!("Sesja „{id}” nie istnieje.")))?;
        Ok(self.session_dto(found).await)
    }

    /// Wysyła `SessionUpdated` z bieżącą postacią sesji.
    pub(crate) async fn announce_session(&self, id: &SessionId) {
        if let Ok(session) = self.session_summary(id).await {
            self.emit(AlfaEvent::SessionUpdated { session });
        }
    }

    /// Sprawdza, że sesja istnieje (i nie jest w koszu).
    pub(crate) fn ensure_session(&self, id: &SessionId) -> Result<(), AppError> {
        let meta = self.inner.sessions.session(id).map_err(AppError::from)?;
        if meta.trashed {
            return Err(AppError::not_found(format!("Sesja „{id}” jest w koszu.")));
        }
        Ok(())
    }
}
