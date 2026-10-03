//! `AppCore::build`: kompozycja modułów w kolejności wyliczonej przez `core-registry`
//! (graf kontraktów z manifestów `module.toml`). Jądro (magistrala, konfiguracja, logi) startuje
//! pierwsze; potem każdy moduł jest budowany i uruchamiany (`Module::start` z magistralą) dokładnie
//! w kolejności `start_order()`, a w rejestrze zostaje jego pośrednik (manifest + zdrowie).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use core_bus_contract::EventBus;
use core_bus_impl::{BroadcastBus, BusConfig};
use core_registry_contract::{
    ContractRef, HealthStatus, Lifecycle, Module, ModuleContext, ModuleManifest, Registry,
};
use core_registry_impl::ModuleRegistry;

use crate::core::AppCore;
use crate::error::AppError;
use crate::infra::proxy::{HealthFn, ProxyModule};
use crate::options::{AppOptions, AppPaths};
use crate::parts::{Built, Kernel};

/// Manifesty modułów (identyfikator → `module.toml`); narzędzia i runtime agentek —
/// `app_agents::MODULES`, scheduler/wyzwalacze/Marszałek — `app_tasks::MODULES`, mosty CLI
/// i MCP — `app_bridges::MODULES`.
const MODULES: &[(&str, &str)] = &[
    ("platform-windows", platform_windows_impl::MODULE_TOML),
    ("device-profile", device_profile_impl::MODULE_TOML),
    ("compliance", compliance_impl::MODULE_TOML),
    ("accounts-hub", accounts_hub_impl::MODULE_TOML),
    ("providers-api", providers_api_impl::MODULE_TOML),
    ("cost-meter", cost_meter_impl::MODULE_TOML),
    ("sessions", sessions_impl::MODULE_TOML),
    ("search", search_impl::MODULE_TOML),
    ("memory", memory_impl::MODULE_TOML),
    ("memory-consolidation", app_memory::CONSOLIDATION_TOML),
    ("artifacts", artifacts_impl::MODULE_TOML),
    ("personas", personas_impl::MODULE_TOML),
    ("model-residency", model_residency_impl::MODULE_TOML),
    ("providers-local", providers_local_impl::MODULE_TOML),
    ("router", router_impl::MODULE_TOML),
    ("risk-classifier", risk_classifier_impl::MODULE_TOML),
    ("safety-broker", safety_broker_impl::MODULE_TOML),
    ("undo-journal", undo_journal_impl::MODULE_TOML),
    ("transfer", transfer_impl::MODULE_TOML),
    ("voice-audio", voice_audio_impl::MODULE_TOML),
    ("voice-tts", voice_tts_impl::MODULE_TOML),
    ("updater", updater_impl::MODULE_TOML),
];

pub(crate) fn internal(what: &str) -> impl Fn(String) -> AppError + '_ {
    move |e| AppError::internal(format!("{what}: {e}"))
}

/// Manifest dla grafu startu z poprawkami kompozycji:
/// - `sessions` wymaga `search-contract` tylko jako indeksera wiązanego później (`with_indexer`
///   + `LateDbProvider`), a `search` wymaga `sessions-contract` — zostaje krawędź search → sessions;
/// - `providers-local` dostarcza `providers-contract` tak jak `providers-api` — obaj są
///   kandydatami Routera, więc w grafie dostawcą kontraktu zostaje `providers-api`;
/// - `safety-broker` w procesie (tryb deweloperski) działa bez `watchdog` (osobny proces).
fn manifest_for_graph(id: &str, toml: &str) -> Result<ModuleManifest, AppError> {
    let mut manifest = ModuleManifest::parse_toml(toml).map_err(|e| internal(id)(e.to_string()))?;
    match id {
        "sessions" => manifest.requires.retain(|c| c.name != "search-contract"),
        "providers-local" => manifest.provides.clear(),
        // Strażniczka używa lokalnego rdzenia Routera (budowany w kroku `router`), a eksport
        // `.alfa` — dokumentów pamięci (`Category::Memory`).
        "memory-consolidation" => manifest.requires.push(contract("router-contract")),
        "transfer" => manifest.requires.push(contract("memory-contract")),
        _ => {}
    }
    // `tools-common-contract` to kontrakt bez modułu (manifest, `Tool`, bramka Brokera),
    // `platform-apps-contract` — porty Office/przeglądarki/rejestru wstrzykiwane przez kompozycję,
    // a role `watchdog-contract` (Job Objects, historia konfiguracji) pełnią Broker w procesie
    // i kompozycja.
    manifest.requires.retain(|c| {
        !matches!(
            c.name.as_str(),
            "tools-common-contract" | "platform-apps-contract" | "watchdog-contract"
        )
    });
    Ok(manifest)
}

fn contract(name: &str) -> ContractRef {
    ContractRef {
        name: name.to_owned(),
        major: 1,
    }
}

/// Gniazdo zdrowia modułu (wypełniane po starcie usługi).
pub(crate) type HealthSlot = Arc<OnceLock<HealthFn>>;

/// Rejestruje pośredników i zwraca kolejność startu + gniazda zdrowia.
async fn plan(
    registry: &ModuleRegistry,
) -> Result<
    (
        Vec<String>,
        BTreeMap<String, HealthSlot>,
        BTreeMap<String, Lifecycle>,
    ),
    AppError,
> {
    let mut slots = BTreeMap::new();
    let mut lifecycles = BTreeMap::new();
    let all = MODULES
        .iter()
        .chain(app_agents::MODULES)
        .chain(app_tasks::MODULES)
        .chain(app_bridges::MODULES)
        .chain(app_gui::MODULES)
        .chain(app_terminal::MODULES)
        .chain(app_skills::MODULES)
        .chain(app_health::MODULES);
    for (id, toml) in all {
        let manifest = manifest_for_graph(id, toml)?;
        lifecycles.insert((*id).to_owned(), manifest.lifecycle);
        let slot: HealthSlot = Arc::new(OnceLock::new());
        let reader = slot.clone();
        let health: HealthFn = Arc::new(move || match reader.get() {
            Some(f) => f(),
            None => HealthStatus::NotStarted,
        });
        registry
            .register(Box::new(ProxyModule::new(manifest, health)))
            .await
            .map_err(|e| internal("rejestr")(e.to_string()))?;
        slots.insert((*id).to_owned(), slot);
    }
    let order = registry
        .start_order()
        .await
        .map_err(|e| internal("kolejność startu")(e.to_string()))?
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    Ok((order, slots, lifecycles))
}

/// Uruchamia usługę (`Module::start` z magistralą) i zwraca ją współdzieloną.
pub(crate) async fn started<T: Module + 'static>(
    mut module: T,
    bus: &Arc<dyn EventBus>,
    slot: Option<&HealthSlot>,
) -> Result<Arc<T>, AppError> {
    let id = module.manifest().id.clone();
    module
        .start(ModuleContext::new(id.clone(), bus.clone()))
        .await
        .map_err(|e| internal(id.as_str())(e.to_string()))?;
    let module = Arc::new(module);
    if let Some(slot) = slot {
        let weak = Arc::downgrade(&module);
        let _ = slot.set(Arc::new(move || match weak.upgrade() {
            Some(m) => m.health(),
            None => HealthStatus::Unhealthy("moduł zwolniony".into()),
        }));
    }
    Ok(module)
}

impl AppCore {
    /// Składa aplikację: jądro → moduły w kolejności z rejestru → stan `AppCore`.
    pub async fn build(paths: AppPaths, options: AppOptions) -> Result<AppCore, AppError> {
        paths.ensure()?;
        let bus: Arc<dyn EventBus> = Arc::new(BroadcastBus::new(BusConfig::default()));
        let registry = Arc::new(ModuleRegistry::new(bus.clone()));
        let (order, slots, lifecycles) = plan(&registry).await?;
        tracing::info!(kolejnosc = ?order, "kolejność startu modułów");
        let mut kernel = Kernel::start(&paths, &options, &bus).await?;
        let mut built = Built::default();
        for id in &order {
            built
                .build_module(id, &paths, &options, &bus, &mut kernel, slots.get(id))
                .await?;
        }
        registry
            .boot()
            .await
            .map_err(|e| internal("start rejestru")(e.to_string()))?;
        for (id, lifecycle) in &lifecycles {
            if *lifecycle != Lifecycle::Always {
                let id = core_registry_contract::ModuleId::new(id.as_str())
                    .map_err(|e| internal("id modułu")(e.to_string()))?;
                registry
                    .activate(&id)
                    .await
                    .map_err(|e| internal("aktywacja modułu")(e.to_string()))?;
            }
        }
        let core = built
            .into_core(paths, options, bus, registry, kernel)
            .await?;
        core.after_start().await;
        Ok(core)
    }

    /// Stan modułów w rejestrze (Ustawienia → Moduły, diagnostyka).
    pub async fn modules(&self) -> Vec<core_registry_contract::ModuleStatus> {
        self.inner.registry.list().await
    }

    /// Kontrakt dostarczany przez moduł (diagnostyka grafu).
    pub async fn provider_of(&self, contract: &str) -> Option<String> {
        let contract = ContractRef {
            name: contract.to_owned(),
            major: 1,
        };
        self.inner
            .registry
            .acquire(&contract)
            .await
            .ok()
            .map(|id| id.to_string())
    }
}
