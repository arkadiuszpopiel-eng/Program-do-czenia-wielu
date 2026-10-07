//! Moduł zgodności v0 (docs/modules/compliance/SPEC.md, PLAN §1.3, §5.5).
//!
//! Ładuje rejestr `docs/compliance/compliance-registry.json` (wbudowany jako domyślny albo
//! z pliku), łączy go z tagami z katalogu dostawców, stosuje wyłączniki użytkownika, degraduje
//! nieświeże trasy do „szarych” i publikuje zdarzenia `compliance.*` na magistralę.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use chrono::NaiveDate;
use compliance_contract::{
    ChangeOrigin, Compliance, ComplianceError, Decision, DenyChecker, DenyLists,
    EVENT_DENYLIST_UPDATED, EVENT_ROUTE_DISABLED, EVENT_ROUTE_ENABLED, EVENT_ROUTE_STALE,
    KernelAuthority, PathEnv, ProviderPolicyInput, Registry, RegistryError, RegistryRoute, RouteId,
    RouteTable, RouteView, SessionTag, TableSettings, Today, event_kind,
};
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Rejestr zgodności z repo (domyślny, wersjonowany razem z kodem).
pub const DEFAULT_REGISTRY_JSON: &str =
    include_str!("../../../docs/compliance/compliance-registry.json");

/// Data lokalna z zegara systemowego.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemToday;

impl Today for SystemToday {
    fn today(&self) -> NaiveDate {
        chrono::Local::now().date_naive()
    }
}

/// Stała data (testy, odtwarzanie).
#[derive(Debug, Clone, Copy)]
pub struct FixedToday(pub NaiveDate);

impl Today for FixedToday {
    fn today(&self) -> NaiveDate {
        self.0
    }
}

/// Konfiguracja modułu (`[compliance]` w TOML).
#[derive(Debug, Clone, Default)]
pub struct ComplianceConfig {
    /// Próg nieświeżości i polityka sesji prywatnych.
    pub table: TableSettings,
    /// Wyłączniki użytkownika `[compliance.routes.<id>] enabled`.
    pub route_overrides: BTreeMap<RouteId, bool>,
    /// Deny-listy; `None` = lista bazowa + `providers[].deny_domains` z rejestru.
    pub deny_lists: Option<DenyLists>,
    /// Zmienne do rozwijania ścieżek (`%USERPROFILE%` itd.).
    pub path_env: PathEnv,
}

impl ComplianceConfig {
    /// Zmienne ścieżek z bieżącego procesu (`USERPROFILE`, `LOCALAPPDATA`, `APPDATA`, `SYSTEMDRIVE`).
    #[must_use]
    pub fn with_process_env(mut self) -> Self {
        let mut env = PathEnv::new();
        for name in ["USERPROFILE", "LOCALAPPDATA", "APPDATA", "SYSTEMDRIVE"] {
            if let Ok(value) = std::env::var(name) {
                env = env.with(name, &value);
            }
        }
        self.path_env = env.derived();
        self
    }
}

/// Błędy budowy modułu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum InitError {
    /// Zepsuty `module.toml`.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// Zepsuty rejestr.
    #[error(transparent)]
    Registry(#[from] RegistryError),
    /// Niepoprawne deny-listy.
    #[error(transparent)]
    DenyList(#[from] ComplianceError),
    /// Błąd odczytu pliku rejestru.
    #[error("nie można odczytać rejestru `{path}`: {reason}")]
    Io {
        /// Ścieżka.
        path: String,
        /// Powód.
        reason: String,
    },
}

/// Wczytuje rejestr z pliku JSON (walidacja formatu i wersji).
pub fn load_registry_file(path: &Path) -> Result<Registry, InitError> {
    let text = std::fs::read_to_string(path).map_err(|e| InitError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    Ok(Registry::from_json(&text)?)
}

/// Rejestr wbudowany (z repo).
pub fn default_registry() -> Result<Registry, RegistryError> {
    Registry::from_json(DEFAULT_REGISTRY_JSON)
}

struct State {
    table: RouteTable,
    deny: DenyChecker,
}

/// Usługa zgodności.
pub struct ComplianceService {
    manifest: ModuleManifest,
    today: Arc<dyn Today>,
    env: PathEnv,
    state: RwLock<State>,
    bus: RwLock<Option<Arc<dyn EventBus>>>,
    ignored_overrides: Vec<RouteId>,
}

impl ComplianceService {
    /// Buduje usługę z rejestru, wpisów katalogu, konfiguracji i źródła daty.
    pub fn new(
        registry: Registry,
        catalog: Vec<ProviderPolicyInput>,
        config: ComplianceConfig,
        today: Arc<dyn Today>,
    ) -> Result<Self, InitError> {
        let manifest = ModuleManifest::parse_toml(MODULE_TOML)?;
        let registry_domains: Vec<String> = registry
            .providers
            .iter()
            .flat_map(|p| p.deny_domains.iter().cloned())
            .collect();
        let lists = config
            .deny_lists
            .unwrap_or_else(|| DenyLists::baseline().with_domains(registry_domains));
        lists.validate()?;
        let mut table = RouteTable::new(registry, catalog, config.table);
        let day = today.today();
        let mut ignored_overrides = Vec::new();
        for (id, on) in &config.route_overrides {
            if table.toggle(id, *on, &ChangeOrigin::User, day).is_err() {
                ignored_overrides.push(id.clone());
            }
        }
        Ok(Self {
            manifest,
            today,
            state: RwLock::new(State {
                table,
                deny: DenyChecker::new(lists, &config.path_env),
            }),
            env: config.path_env,
            bus: RwLock::new(None),
            ignored_overrides,
        })
    }

    /// Usługa z wbudowanym rejestrem, zegarem systemowym i zmiennymi ścieżek procesu.
    pub fn with_default_registry(catalog: Vec<ProviderPolicyInput>) -> Result<Self, InitError> {
        Self::new(
            default_registry()?,
            catalog,
            ComplianceConfig::default().with_process_env(),
            Arc::new(SystemToday),
        )
    }

    /// Wyłączniki z konfiguracji odrzucone przy starcie (nieznana albo zabroniona trasa).
    pub fn ignored_overrides(&self) -> &[RouteId] {
        &self.ignored_overrides
    }

    /// Trasy zdegradowane dziś z powodu nieświeżego rejestru.
    pub fn stale_routes(&self) -> Vec<RouteId> {
        self.read().table.stale_routes(self.today.today())
    }

    /// Publikuje `compliance.route.stale` dla każdej nieświeżej trasy; zwraca ich listę.
    pub async fn announce_stale(&self) -> Vec<RouteId> {
        let stale = self.stale_routes();
        let max_age = self.read().table.max_age_days();
        for id in &stale {
            let payload = serde_json::json!({ "route": id.as_str(), "max_age_days": max_age });
            self.publish(EVENT_ROUTE_STALE, Level::Warn, payload).await;
        }
        stale
    }

    fn read(&self) -> RwLockReadGuard<'_, State> {
        self.state.read().unwrap_or_else(|p| p.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, State> {
        self.state.write().unwrap_or_else(|p| p.into_inner())
    }

    fn bus(&self) -> Option<Arc<dyn EventBus>> {
        self.bus.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    async fn publish(&self, name: &str, level: Level, payload: serde_json::Value) {
        if let Some(bus) = self.bus() {
            // Zdarzenie jest informacyjne; błąd magistrali nie cofa zmiany stanu.
            let _ = bus
                .publish(Event::new(event_kind(name), level, payload))
                .await;
        }
    }
}

#[async_trait]
impl Compliance for ComplianceService {
    fn route(&self, id: &RouteId) -> Option<RegistryRoute> {
        self.read().table.registry().route(id).cloned()
    }

    fn view(&self, id: &RouteId) -> Option<RouteView> {
        self.read().table.view(id, self.today.today())
    }

    fn views(&self) -> Vec<RouteView> {
        self.read().table.views(self.today.today())
    }

    fn route_allowed(&self, id: &RouteId, session: SessionTag) -> Decision {
        self.read().table.decide(id, session, self.today.today())
    }

    async fn set_enabled(
        &self,
        id: &RouteId,
        on: bool,
        origin: ChangeOrigin,
    ) -> Result<RouteView, ComplianceError> {
        let (before, after) = self
            .write()
            .table
            .toggle(id, on, &origin, self.today.today())?;
        if before.enabled != after.enabled || before.user_override != after.user_override {
            let name = if on {
                EVENT_ROUTE_ENABLED
            } else {
                EVENT_ROUTE_DISABLED
            };
            let payload = serde_json::json!({
                "route": id.as_str(),
                "origin": origin,
                "status": after.effective.status,
                "stale": after.effective.stale,
            });
            self.publish(name, Level::Info, payload).await;
        }
        Ok(after)
    }

    fn deny_lists(&self) -> DenyLists {
        self.read().deny.lists().clone()
    }

    fn is_denied_path(&self, path: &str) -> bool {
        self.read().deny.is_denied_path(path, &self.env)
    }

    fn is_denied_domain(&self, domain: &str) -> bool {
        self.read().deny.is_denied_domain(domain)
    }

    async fn replace_deny_lists(
        &self,
        _authority: &KernelAuthority,
        lists: DenyLists,
    ) -> Result<(), ComplianceError> {
        lists.validate()?;
        let version = lists.version;
        self.write().deny = DenyChecker::new(lists, &self.env);
        let payload = serde_json::json!({ "version": version });
        self.publish(EVENT_DENYLIST_UPDATED, Level::Warn, payload)
            .await;
        Ok(())
    }
}

#[async_trait]
impl Module for ComplianceService {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        {
            let mut bus = self.bus.write().unwrap_or_else(|p| p.into_inner());
            if bus.is_some() {
                return Err(ModuleError::AlreadyStarted);
            }
            *bus = Some(ctx.bus);
        }
        self.announce_stale().await;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .map(|_| ())
            .ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        if self.bus().is_none() {
            return HealthStatus::NotStarted;
        }
        match self.stale_routes().len() {
            0 => HealthStatus::Healthy,
            n => HealthStatus::Degraded(format!("{n} tras wymaga ponownej weryfikacji")),
        }
    }
}
