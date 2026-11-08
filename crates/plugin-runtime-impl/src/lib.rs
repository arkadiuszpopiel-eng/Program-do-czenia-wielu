//! Implementacja modułu `plugin-runtime` (docs/modules/plugin-runtime/SPEC.md, ADR 0012,
//! THREAT_MODEL S09, ACCEPTANCE F8-05) na **wasmtime 36 LTS** (komponenty, własny WIT).
//!
//! Piaskownica: brak WASI (jedyny dozwolony import to `alfa:plugin/host@0.1.0`), paliwo
//! i przerwanie epokowe per wywołanie, limiter pamięci/tabel/instancji, stos 512 KiB, limity
//! wejścia/wyjścia i liczby operacji hosta; **nowa instancja na każde wywołanie**; pułapka =
//! czytelny [`plugin_runtime_contract::ExecError`], nigdy panika hosta. Moduł jest ładowany
//! wyłącznie po weryfikacji SHA-256 z zatwierdzonego manifestu. Operacje hosta idą przez
//! Brokera z podmiotem agentki wywołującej. Cykl życia z kontraktu ([`PluginLibrary`]),
//! trwały magazyn ([`DirPluginStore`]), zdarzenia `plugin.*` na magistrali.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod engine;
mod hostcall;
mod inner;
mod sandbox;
mod store_dir;
mod tool;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use plugin_runtime_contract::{
    LoadError, PluginApproval, PluginError, PluginHost, PluginId, PluginLibrary, PluginManifest,
    PluginRecord, PluginSource, PluginStore, Plugins, check_wasm,
};
use safety_broker_contract::Broker;
use semver::Version;
use tools_common_contract::{BrokerGate, Tool, ToolManifest, Toolset};

pub use engine::MAX_WASM_STACK;
pub use store_dir::DirPluginStore;
pub use tool::PluginTool;

use crate::engine::SandboxEngine;
pub(crate) use crate::inner::Inner;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu.
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Konfiguracja (`[plugins]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Krok zegara epok (dokładność limitu czasu i anulowania).
    pub tick: Duration,
    /// Najwięcej równoległych wywołań (pamięć: ≤ N × limit pamięci wtyczki).
    pub max_concurrent: usize,
    /// TTL tokenu operacji hosta (ms; token unieważniany zaraz po operacji).
    pub token_ttl_ms: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            tick: Duration::from_millis(10),
            max_concurrent: 4,
            token_ttl_ms: 60_000,
        }
    }
}

/// Zależności.
#[derive(Clone)]
pub struct PluginDeps {
    /// Broker (tokeny operacji hosta dla agentki wywołującej).
    pub broker: Arc<dyn Broker>,
    /// Port wykonujący operacje hosta (pliki, sieć).
    pub host: Arc<dyn PluginHost>,
    /// Magazyn rekordów i modułów.
    pub store: Arc<dyn PluginStore>,
    /// Magistrala (zdarzenia `plugin.*`; ustawiana też przy starcie modułu).
    pub bus: Option<Arc<dyn EventBus>>,
    /// Konfiguracja.
    pub config: RuntimeConfig,
}

fn system_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Moduł wtyczek: biblioteka + piaskownica + narzędzia dla agentek.
pub struct PluginRuntime {
    manifest: ModuleManifest,
    inner: Arc<Inner>,
    started: bool,
}

impl PluginRuntime {
    /// Moduł nad zależnościami (rekordy wczytane z magazynu).
    pub fn new(deps: PluginDeps) -> Result<Self, String> {
        Self::with_clock(deps, system_now_ms)
    }

    /// Jak [`PluginRuntime::new`], z zegarem (ms; testy).
    pub fn with_clock(deps: PluginDeps, clock: fn() -> u64) -> Result<Self, String> {
        let records = deps.store.load_records()?;
        let inner = Inner {
            engine: SandboxEngine::new(deps.config.tick)?,
            gate: BrokerGate::new(deps.broker),
            host: deps.host,
            store: deps.store,
            lib: Mutex::new(PluginLibrary::new(records)),
            cache: Mutex::new(BTreeMap::new()),
            bus: RwLock::new(deps.bus),
            permits: tokio::sync::Semaphore::new(deps.config.max_concurrent.max(1)),
            config: deps.config,
            clock,
        };
        Ok(Self {
            manifest: module_manifest().map_err(|e| e.to_string())?,
            inner: Arc::new(inner),
            started: false,
        })
    }

    /// Krok zegara epok (diagnostyka).
    pub fn tick(&self) -> Duration {
        self.inner.engine.tick()
    }

    /// Kontrola modułu bez instalacji (np. podgląd w UI przed propozycją).
    pub async fn inspect(&self, wasm: Vec<u8>) -> Result<(), LoadError> {
        self.inner.compile(wasm).await.map(|_| ())
    }
}

#[async_trait]
impl Module for PluginRuntime {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if self.started {
            return Err(ModuleError::AlreadyStarted);
        }
        *self.inner.bus.write().unwrap_or_else(|p| p.into_inner()) = Some(ctx.bus);
        self.started = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        if !self.started {
            return Err(ModuleError::NotStarted);
        }
        lock(&self.inner.cache).clear();
        self.started = false;
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if self.started {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}

impl Toolset for PluginRuntime {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        Plugins::tools(self)
    }
}

#[async_trait]
impl Plugins for PluginRuntime {
    fn list(&self) -> Vec<PluginRecord> {
        self.inner.lib().records().to_vec()
    }

    fn installed(&self, id: &PluginId) -> Option<PluginRecord> {
        self.inner.lib().installed(id).cloned()
    }

    fn tool_catalog(&self) -> Vec<ToolManifest> {
        self.inner.lib().tool_catalog()
    }

    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        let lib = self.inner.lib();
        lib.active()
            .flat_map(|r| {
                r.manifest.tools.iter().map(|d| {
                    Arc::new(PluginTool::new(
                        self.inner.clone(),
                        r.manifest.id.clone(),
                        d.clone(),
                        r.manifest.tool_manifest(d),
                    )) as Arc<dyn Tool>
                })
            })
            .collect()
    }

    async fn propose(
        &self,
        manifest: PluginManifest,
        wasm: Vec<u8>,
        source: PluginSource,
    ) -> Result<PluginRecord, PluginError> {
        plugin_runtime_contract::validate_manifest(&manifest)?;
        check_wasm(&manifest, &wasm)?;
        self.inner
            .compile(wasm.clone())
            .await
            .map_err(PluginError::Load)?;
        let sha = manifest.wasm_sha256.clone();
        self.inner
            .store
            .put_wasm(&sha, &wasm)
            .map_err(PluginError::Store)?;
        let result = self
            .inner
            .change(|l, now| l.propose(manifest, source, now))
            .await;
        if result.is_err() && !self.inner.lib().uses_wasm(&sha) {
            let _ = self.inner.store.delete_wasm(&sha);
        }
        result
    }

    async fn approve(
        &self,
        id: &PluginId,
        version: &Version,
        approval: PluginApproval,
    ) -> Result<PluginRecord, PluginError> {
        let record = self
            .list()
            .into_iter()
            .find(|r| &r.manifest.id == id && &r.manifest.version == version)
            .ok_or_else(|| PluginError::NotFound(format!("{id}@{version}")))?;
        // Bajty ponownie z magazynu: podmiana między przeglądem a zatwierdzeniem = odmowa.
        self.inner.evict(std::slice::from_ref(&record));
        self.inner
            .prepared(&record)
            .await
            .map_err(PluginError::Load)?;
        self.inner
            .change(|l, now| l.approve(id, version, approval, now))
            .await
    }

    async fn reject(&self, id: &PluginId, version: &Version) -> Result<PluginRecord, PluginError> {
        self.inner.change(|l, now| l.reject(id, version, now)).await
    }

    async fn disable(&self, id: &PluginId) -> Result<PluginRecord, PluginError> {
        let r = self.inner.change(|l, now| l.disable(id, now)).await?;
        self.inner.evict(std::slice::from_ref(&r));
        Ok(r)
    }

    async fn enable(
        &self,
        id: &PluginId,
        approval: PluginApproval,
    ) -> Result<PluginRecord, PluginError> {
        self.inner
            .change(|l, now| l.enable(id, approval, now))
            .await
    }

    async fn remove(&self, id: &PluginId) -> Result<Vec<PluginRecord>, PluginError> {
        let gone = self.inner.change(|l, _| l.remove(id)).await?;
        self.inner.evict(&gone);
        let lib = self.inner.lib().clone();
        for r in &gone {
            if !lib.uses_wasm(&r.manifest.wasm_sha256) {
                self.inner
                    .store
                    .delete_wasm(&r.manifest.wasm_sha256)
                    .map_err(PluginError::Store)?;
            }
        }
        Ok(gone)
    }
}
