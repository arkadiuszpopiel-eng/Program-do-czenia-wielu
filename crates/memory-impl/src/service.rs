//! Moduł pamięci F7: silnik `memory-contract` nad [`SqliteBackend`], zdarzenia na magistralę,
//! identyfikatory UUIDv7, prywatność z katalogu sesji.

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::Level;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use memory_contract::{
    EnginePorts, EventSink, IdSource, MemoryEngine, PrivacyOracle, SessionId, events as names,
};
use search_contract::{TxIndexer, TxSearcher};
use sessions_contract::{PrivacyTag, SessionCatalog};

use crate::backend::SqliteBackend;
use crate::events::Outbox;
use crate::scopes::ScopeDbs;

/// Pamięć F7 na bazach SQLCipher.
pub type SqliteMemoryService = MemoryEngine<SqliteBackend>;

/// Identyfikatory `<prefiks>-<uuidv7>` (porządek czasowy, bez kolizji między maszynami).
#[derive(Debug, Clone, Copy, Default)]
pub struct UuidIds;

impl IdSource for UuidIds {
    fn next_id(&self, prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::now_v7().simple())
    }
}

/// Prywatność z katalogu sesji: `private` i `local_only` → prywatna; sesja nieznana → prywatna
/// (bezpieczniej: brak awansu).
pub struct CatalogPrivacy {
    catalog: Arc<dyn SessionCatalog>,
}

impl CatalogPrivacy {
    /// Nowa wyrocznia prywatności.
    pub fn new(catalog: Arc<dyn SessionCatalog>) -> Self {
        Self { catalog }
    }
}

impl PrivacyOracle for CatalogPrivacy {
    fn is_private(&self, session: &SessionId) -> bool {
        self.catalog
            .session(session)
            .map_or(true, |meta| meta.privacy != PrivacyTag::Normal)
    }
}

/// Zdarzenia silnika → kolejka modułu (`memory.recalled` na poziomie Debug, reszta Info).
struct OutboxSink(Arc<Outbox>);

impl EventSink for OutboxSink {
    fn emit(&self, kind: &str, session: Option<&SessionId>, payload: serde_json::Value) {
        let level = if kind == names::RECALLED {
            Level::Debug
        } else {
            Level::Info
        };
        self.0.emit(kind, level, session, payload);
    }
}

/// Zależności modułu F7 (składa je korzeń kompozycji `app-*`).
#[derive(Clone)]
pub struct MemoryParts {
    /// Bazy zakresów (`VaultScopeDbs`).
    pub dbs: Arc<dyn ScopeDbs>,
    /// Indeksowanie w transakcji (`search_impl::SqliteSearch`).
    pub indexer: Arc<dyn TxIndexer>,
    /// Zapytania w połączeniu (ten sam `SqliteSearch`).
    pub searcher: Arc<dyn TxSearcher>,
    /// Prywatność sesji ([`CatalogPrivacy`]).
    pub privacy: Arc<dyn PrivacyOracle>,
}

/// Moduł `memory` F7 (rejestr modułów): silnik + zdarzenia + manifest.
pub struct MemoryModule {
    service: Arc<SqliteMemoryService>,
    outbox: Arc<Outbox>,
    manifest: ModuleManifest,
}

impl MemoryModule {
    /// Moduł z portami produkcyjnymi (zegar systemowy, UUIDv7, reranker heurystyczny).
    pub fn new(parts: MemoryParts) -> Result<Self, ManifestError> {
        let ports = EnginePorts::system(Arc::new(UuidIds), parts.privacy.clone());
        Self::with_ports(parts, ports)
    }

    /// Moduł z podanymi portami (testy: zegar wirtualny, reranker modelowy); zdarzenia zawsze
    /// idą na magistralę modułu.
    pub fn with_ports(parts: MemoryParts, mut ports: EnginePorts) -> Result<Self, ManifestError> {
        let outbox = Arc::new(Outbox::default());
        ports.events = Arc::new(OutboxSink(outbox.clone()));
        ports.privacy = parts.privacy;
        let backend = SqliteBackend::new(parts.dbs, parts.indexer, parts.searcher);
        Ok(Self {
            service: Arc::new(MemoryEngine::new(backend, ports)),
            outbox,
            manifest: ModuleManifest::parse_toml(crate::MODULE_TOML)?,
        })
    }

    /// Pamięć (dla `app-*`: komendy Inspektora, narzędzia agentek, konsolidacja, `transfer`).
    pub fn service(&self) -> Arc<SqliteMemoryService> {
        Arc::clone(&self.service)
    }
}

#[async_trait]
impl Module for MemoryModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        self.outbox.start(ctx.bus)
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.outbox.stop()
    }

    fn health(&self) -> HealthStatus {
        if self.outbox.started() {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}
