//! Implementacja modułu `sessions` (docs/modules/sessions/SPEC.md, ADR 0006, ADR 0008).
//!
//! Układ na dysku (`data_dir`, domyślnie `%LOCALAPPDATA%\Alfa\sessions`):
//! - `index.db` — katalog sesji (metadane, liczniki), szyfrowany kluczem [`INDEX_KEY_NAME`];
//! - `<id>.db` — osobna baza każdej sesji (historia, gałęzie, szkic; tabele `search`/`memory`/
//!   `artifacts` w tym samym pliku), szyfrowana własnym kluczem z [`KeyVault`].
//!
//! Historia jest append-only także na poziomie bazy (wyzwalacze odrzucają `UPDATE`/`DELETE`).
//! Usunięcie sesji = usunięcie klucza z sejfu (crypto-shredding) + plików bazy + wpisu katalogu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod catalog;
mod events;
#[cfg(feature = "dev-file-vault")]
mod file_vault;
mod history;
mod rows;
mod schema;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use core_registry_contract::{ManifestError, ModuleManifest};
use lib_sqlstore::{Db, migrate, rusqlite::OptionalExtension};
use search_contract::TxIndexer;
use sessions_contract::{
    INDEX_KEY_NAME, KeyVault, SessionDbProvider, SessionError, SessionId, load_or_create_key,
    session_key_name,
};

#[cfg(feature = "dev-file-vault")]
pub use file_vault::FileKeyVault;
pub use schema::{INDEX_MIGRATIONS, SESSION_MIGRATIONS};

use crate::events::Outbox;
use crate::rows::db_err;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Konfiguracja (`[sessions]` w TOML; ścieżki rozwinięte przez wywołującego).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionsConfig {
    /// Katalog baz (`%LOCALAPPDATA%\Alfa\sessions`) — **wyłączny dla modułu**: pliki `*.db` bez wpisu
    /// w katalogu sesji są przy otwarciu usuwane jako sieroty po przerwanym usuwaniu.
    pub data_dir: PathBuf,
    /// Korzeń katalogów roboczych (`%USERPROFILE%\Alfa\Sesje`).
    pub workdir_root: PathBuf,
}

/// Moduł sesji na SQLCipher.
pub struct SqliteSessions {
    config: SessionsConfig,
    vault: Arc<dyn KeyVault>,
    index: Db,
    open: Mutex<BTreeMap<SessionId, Arc<Db>>>,
    active: Mutex<BTreeSet<SessionId>>,
    indexer: Option<Arc<dyn TxIndexer>>,
    outbox: Outbox,
    manifest: ModuleManifest,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl SqliteSessions {
    /// Otwiera (tworzy) katalog sesji; usuwa osierocone pliki baz bez wpisu w katalogu.
    pub fn open(config: SessionsConfig, vault: Arc<dyn KeyVault>) -> Result<Self, SessionError> {
        let manifest = ModuleManifest::parse_toml(MODULE_TOML).map_err(manifest_err)?;
        let key = load_or_create_key(vault.as_ref(), INDEX_KEY_NAME)?;
        let index =
            Db::open(&config.data_dir.join("index.db"), &key).map_err(SessionError::storage)?;
        index
            .with(|c| migrate(c, schema::INDEX_NAMESPACE, schema::INDEX_MIGRATIONS).map(|_| ()))
            .map_err(SessionError::storage)?;
        let sessions = Self {
            config,
            vault,
            index,
            open: Mutex::new(BTreeMap::new()),
            active: Mutex::new(BTreeSet::new()),
            indexer: None,
            outbox: Outbox::default(),
            manifest,
        };
        sessions.sweep_orphans()?;
        Ok(sessions)
    }

    /// Dołącza indeksowanie tur (`search`) w transakcji zapisu tury.
    #[must_use]
    pub fn with_indexer(mut self, indexer: Arc<dyn TxIndexer>) -> Self {
        self.indexer = Some(indexer);
        self
    }

    /// Konfiguracja.
    pub fn config(&self) -> &SessionsConfig {
        &self.config
    }

    /// Ścieżka pliku bazy sesji.
    pub fn session_path(&self, id: &SessionId) -> PathBuf {
        self.config.data_dir.join(format!("{id}.db"))
    }

    /// Liczba otwartych baz sesji (budżet RAM).
    pub fn open_count(&self) -> usize {
        lock(&self.open).len()
    }

    /// Zamyka bazę sesji (np. po zamknięciu karty); ponowne użycie otworzy ją znowu.
    pub fn close_session(&self, id: &SessionId) {
        let db = lock(&self.open).remove(id);
        if let Some(db) = db.and_then(|db| Arc::try_unwrap(db).ok()) {
            let _ = db.close();
        }
    }

    fn exists(&self, id: &SessionId) -> Result<(), SessionError> {
        let found = self
            .index
            .with(|c| {
                c.query_row(
                    "SELECT 1 FROM sessions WHERE id = ?1",
                    [id.as_str()],
                    |_| Ok(()),
                )
                .optional()
            })
            .map_err(db_err)?;
        found.ok_or_else(|| SessionError::NotFound { id: id.clone() })
    }

    /// Otwiera bazę sesji kluczem, migruje schemat i tabele indeksu (bez wstawiania do pamięci
    /// podręcznej — robi to wywołujący, trzymając blokadę `open`).
    fn open_db(&self, id: &SessionId, key: &lib_sqlstore::DbKey) -> Result<Arc<Db>, SessionError> {
        let db = Db::open(&self.session_path(id), key).map_err(SessionError::storage)?;
        db.with(|c| {
            c.execute_batch(&format!(
                "PRAGMA cache_size = -{};",
                schema::SESSION_CACHE_KIB
            ))
            .map_err(SessionError::storage)?;
            migrate(c, schema::SESSION_NAMESPACE, schema::SESSION_MIGRATIONS)
                .map_err(SessionError::storage)?;
            if let Some(indexer) = &self.indexer {
                indexer.prepare(c).map_err(SessionError::storage)?;
            }
            Ok::<(), SessionError>(())
        })?;
        Ok(Arc::new(db))
    }

    /// Usuwa pliki `*.db` w `data_dir`, których nie ma w katalogu (np. po przerwanym usuwaniu —
    /// klucz już nie istnieje, więc to tylko sprzątanie szyfrogramu).
    fn sweep_orphans(&self) -> Result<usize, SessionError> {
        let known: BTreeSet<String> = self
            .session_ids()?
            .iter()
            .map(|s| format!("{s}.db"))
            .collect();
        let entries = match std::fs::read_dir(&self.config.data_dir) {
            Ok(e) => e,
            Err(e) => return Err(SessionError::storage(e)),
        };
        let mut removed = 0;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".db") && name != "index.db" && !known.contains(&name) {
                removed += lib_sqlstore::remove_database(&entry.path())
                    .map_err(SessionError::storage)?
                    .len();
            }
        }
        Ok(removed)
    }
}

fn manifest_err(e: ManifestError) -> SessionError {
    SessionError::storage(format!("module.toml: {e}"))
}

impl SessionDbProvider for SqliteSessions {
    fn session_db(&self, id: &SessionId) -> Result<Arc<Db>, SessionError> {
        // Blokada `open` przez całe otwieranie: jedna instancja `Db` na plik. Kolejność blokad:
        // `open` → `index` (nigdy odwrotnie).
        let mut open = lock(&self.open);
        if let Some(db) = open.get(id) {
            return Ok(Arc::clone(db));
        }
        self.exists(id)?;
        let key = self
            .vault
            .load(&session_key_name(id))?
            .ok_or_else(|| SessionError::Vault {
                reason: format!("brak klucza sesji {id}"),
            })?;
        let db = self.open_db(id, &key)?;
        open.insert(id.clone(), Arc::clone(&db));
        Ok(db)
    }

    fn session_ids(&self) -> Result<Vec<SessionId>, SessionError> {
        self.index
            .with(|c| {
                let mut stmt = c.prepare("SELECT id FROM sessions ORDER BY id")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
                rows.map(|r| r.map(SessionId::new)).collect()
            })
            .map_err(db_err)
    }
}
