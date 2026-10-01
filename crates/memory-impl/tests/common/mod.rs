//! Wspólne narzędzia testów F7: prawdziwe bazy SQLCipher (sesje: `TempDbProvider`, zakresy własne:
//! `VaultScopeDbs` z sejfem w pamięci), indeks atrapy `FakeSearch` (FTS po prefiksach + wektory
//! `HashEmbedder`, RRF) jako `TxIndexer` i `TxSearcher`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::ops::Deref;
use std::sync::Arc;

use memory_contract::{EnginePorts, MemoryEngine};
use memory_impl::{SqliteBackend, SqliteMemoryService, VaultScopeDbs};
use search_fake::FakeSearch;
use sessions_fake::{MemoryKeyVault, TempDbProvider};

/// Pełny stos pamięci F7 na bazach tymczasowych.
pub struct Stack {
    pub provider: Arc<TempDbProvider>,
    pub vault: Arc<MemoryKeyVault>,
    pub index: Arc<FakeSearch>,
    pub dbs: Arc<VaultScopeDbs>,
    pub memory: Arc<SqliteMemoryService>,
}

impl Deref for Stack {
    type Target = SqliteMemoryService;

    fn deref(&self) -> &SqliteMemoryService {
        &self.memory
    }
}

/// Stos z podanymi portami (testy kontraktowe: zegar wirtualny, prywatność, zdarzenia).
pub fn stack_with(ports: EnginePorts) -> Stack {
    let provider = Arc::new(TempDbProvider::new().unwrap());
    let vault = Arc::new(MemoryKeyVault::new());
    let index = Arc::new(FakeSearch::new());
    let dbs = Arc::new(VaultScopeDbs::new(
        provider.dir().join("memory"),
        vault.clone(),
        provider.clone(),
    ));
    let backend = SqliteBackend::new(dbs.clone(), index.clone(), index.clone());
    Stack {
        provider,
        vault,
        index,
        dbs,
        memory: Arc::new(MemoryEngine::new(backend, ports)),
    }
}

/// Stos z portami deterministycznymi.
pub fn stack() -> Stack {
    stack_with(EnginePorts::deterministic())
}
