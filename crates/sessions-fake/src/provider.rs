//! Dostawca prawdziwych, szyfrowanych baz sesji w katalogu tymczasowym (dla testów innych modułów).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lib_sqlstore::{Db, DbKey};
use sessions_contract::{SessionDbProvider, SessionError, SessionId};
use tempfile::TempDir;

/// Bazy `<tmp>/<id>.db` otwierane na żądanie z losowym kluczem; katalog znika przy `drop`.
///
/// Każda sesja, o którą zapytano, „istnieje” (atrapa nie prowadzi katalogu sesji).
pub struct TempDbProvider {
    dir: TempDir,
    dbs: Mutex<BTreeMap<SessionId, Arc<Db>>>,
}

impl TempDbProvider {
    /// Nowy dostawca w świeżym katalogu tymczasowym.
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            dir: tempfile::tempdir()?,
            dbs: Mutex::new(BTreeMap::new()),
        })
    }

    /// Katalog z bazami.
    pub fn dir(&self) -> &std::path::Path {
        self.dir.path()
    }

    /// Zapomina bazę sesji (symulacja usunięcia sesji; plik zostaje usunięty).
    pub fn drop_session(&self, id: &SessionId) -> Result<(), SessionError> {
        let removed = self
            .dbs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
        if let Some(db) = removed {
            let path = db.path().to_path_buf();
            drop(db);
            lib_sqlstore::remove_database(&path).map_err(SessionError::storage)?;
        }
        Ok(())
    }
}

impl SessionDbProvider for TempDbProvider {
    fn session_db(&self, id: &SessionId) -> Result<Arc<Db>, SessionError> {
        let mut dbs = self.dbs.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(db) = dbs.get(id) {
            return Ok(Arc::clone(db));
        }
        let key = DbKey::generate().map_err(SessionError::storage)?;
        let path = self.dir.path().join(format!("{id}.db"));
        let db = Arc::new(Db::open(&path, &key).map_err(SessionError::storage)?);
        dbs.insert(id.clone(), Arc::clone(&db));
        Ok(db)
    }

    fn session_ids(&self) -> Result<Vec<SessionId>, SessionError> {
        Ok(self
            .dbs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect())
    }
}
