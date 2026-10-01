//! Pamięć F7 w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit rozmiaru crate'a):
//! - [`memory_module`] — `MemoryModule` (`memory-impl`): zakres sesji w bazie sesji,
//!   projekt/agentka/globalna w osobnych szyfrowanych bazach `%LOCALAPPDATA%\Alfa\memory`
//!   z kluczami w sejfie (`VaultScopeDbs`), prywatność z katalogu sesji (`CatalogPrivacy`);
//! - [`MemoryApp`] — komendy Inspektora (`memory_*`, DTO `app-api`), „zapamiętaj" w każdym
//!   zakresie, zapomnienie sesji przed crypto-shreddingiem, kontekst pamięci czatu;
//! - [`forget`] — podgląd kaskady przed zapomnieniem;
//! - [`tools`] — narzędzia `memory_recall` / `memory_remember` dla agentek (`Accessor::Agent`
//!   z ról obsady i projektu sesji — [`RoleAccess`]);
//! - [`guardian`] — Strażniczka pamięci (`ConsolidationModule`: model lokalny przez Router,
//!   budżet tła, stan maszyny; licznik bezczynności — port z atrapą „nigdy bezczynny").

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod access;
mod app;
pub mod forget;
pub mod guardian;
pub mod ids;
pub mod map;
pub mod tools;

use std::path::PathBuf;
use std::sync::Arc;

use app_api::AppError;
use memory_contract::PrivacyOracle;
use memory_impl::{CatalogPrivacy, MemoryModule, MemoryParts, VaultScopeDbs};
use search_contract::{TxIndexer, TxSearcher};
use sessions_contract::{KeyVault, SessionCatalog, SessionDbProvider};

pub use access::{RoleAccess, role_grants};
pub use app::{CONTEXT_BUDGET, MemoryApp, MemoryAppParts};
pub use memory_impl::{MODULE_TOML, MemoryDocuments};

/// Manifest Strażniczki pamięci (rejestr modułów w `app-core`).
pub const CONSOLIDATION_TOML: &str = memory_consolidation_impl::MODULE_TOML;

/// Moduł pamięci F7 i wyrocznia prywatności sesji (wspólna z eksportem `.alfa`).
pub fn memory_module<S, X>(
    root: PathBuf,
    vault: Arc<dyn KeyVault>,
    sessions: Arc<S>,
    search: Arc<X>,
) -> Result<(MemoryModule, Arc<dyn PrivacyOracle>), AppError>
where
    S: SessionDbProvider + SessionCatalog + 'static,
    X: TxIndexer + TxSearcher + 'static,
{
    let provider: Arc<dyn SessionDbProvider> = sessions.clone();
    let catalog: Arc<dyn SessionCatalog> = sessions;
    let privacy: Arc<dyn PrivacyOracle> = Arc::new(CatalogPrivacy::new(catalog));
    let parts = MemoryParts {
        dbs: Arc::new(VaultScopeDbs::new(root, vault, provider)),
        indexer: search.clone(),
        searcher: search,
        privacy: privacy.clone(),
    };
    let module =
        MemoryModule::new(parts).map_err(|e| AppError::internal(format!("memory: {e}")))?;
    Ok((module, privacy))
}

/// Zdarzenia `memory.*` z magistrali (zapis agentki, porządkowanie, import) → `MemoryChanged`
/// dla Inspektora (bez treści; `memory.recalled` pomijane).
pub async fn spawn_bridge(bus: Arc<dyn core_bus_contract::EventBus>, events: app_api::EventHub) {
    use core_bus_contract::{BusItem, EventFilter};
    use futures_util::StreamExt;
    let Ok(mut stream) = bus.subscribe(EventFilter::prefix("memory.")).await else {
        tracing::warn!("most zdarzeń pamięci niedostępny");
        return;
    };
    tokio::spawn(async move {
        while let Some(item) = stream.next().await {
            let BusItem::Event(event) = item else {
                continue;
            };
            if event.kind.as_str() == memory_contract::events::RECALLED {
                continue;
            }
            events.emit(app_api::dto::AlfaEvent::MemoryChanged {
                scope_key: event.session.as_ref().map(|s| format!("session:{s}")),
            });
        }
    });
}
