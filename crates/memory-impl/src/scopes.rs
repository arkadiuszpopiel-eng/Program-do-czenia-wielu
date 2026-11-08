//! Bazy zakresów: sesja → baza sesji (`SessionDbProvider`, crypto-shredding robi `sessions`);
//! projekt/agentka/globalna → **osobne szyfrowane bazy** w katalogu pamięci z kluczem w sejfie
//! (`alfa/memory/<zakres>`), usuwane przez crypto-shredding (klucz + pliki z `-wal`/`-shm`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lib_sqlstore::{Db, remove_database};
use memory_contract::{AgentId, MemoryError, MemoryScope, validate_scope};
use sessions_contract::{KeyVault, SessionDbProvider, SessionError, load_or_create_key};

/// Dostęp do baz zakresów.
pub trait ScopeDbs: Send + Sync {
    /// Baza zakresu; `create = false` → `None`, gdy baza jeszcze nie istnieje (odczyt nie tworzy
    /// pustych plików).
    fn db(&self, scope: &MemoryScope, create: bool) -> Result<Option<Arc<Db>>, MemoryError>;
    /// Crypto-shredding bazy własnej (projekt/agentka/globalna); `true`, gdy coś usunięto.
    /// Zakres sesji → błąd (bazę sesji usuwa moduł `sessions`).
    fn shred(&self, scope: &MemoryScope) -> Result<bool, MemoryError>;
    /// Zakresy, które mogą mieć dane: bazy własne na dysku i wszystkie sesje.
    fn known(&self) -> Result<Vec<MemoryScope>, MemoryError>;
}

/// Nazwa klucza bazy zakresu w sejfie.
pub fn scope_key_name(scope: &MemoryScope) -> Option<String> {
    match scope {
        MemoryScope::Global => Some("alfa/memory/global".into()),
        MemoryScope::Project(p) => Some(format!("alfa/memory/project/{p}")),
        MemoryScope::Agent(a) => Some(format!("alfa/memory/agent/{a}")),
        MemoryScope::Session(_) => None,
    }
}

/// Nazwa pliku bazy zakresu (identyfikatory zwalidowane: `[A-Za-z0-9_-]{1,64}`).
pub fn scope_file_name(scope: &MemoryScope) -> Option<String> {
    match scope {
        MemoryScope::Global => Some("global.db".into()),
        MemoryScope::Project(p) => Some(format!("project-{p}.db")),
        MemoryScope::Agent(a) => Some(format!("agent-{a}.db")),
        MemoryScope::Session(_) => None,
    }
}

fn scope_of_file(name: &str) -> Option<MemoryScope> {
    let stem = name.strip_suffix(".db")?;
    let scope = if stem == "global" {
        MemoryScope::Global
    } else if let Some(p) = stem.strip_prefix("project-") {
        MemoryScope::Project(p.to_owned())
    } else if let Some(a) = stem.strip_prefix("agent-") {
        MemoryScope::Agent(AgentId::new(a))
    } else {
        return None;
    };
    validate_scope(&scope).ok().map(|()| scope)
}

/// Bazy zakresów w katalogu `root` z kluczami w sejfie.
pub struct VaultScopeDbs {
    root: PathBuf,
    vault: Arc<dyn KeyVault>,
    sessions: Arc<dyn SessionDbProvider>,
    open: Mutex<BTreeMap<MemoryScope, Arc<Db>>>,
}

impl VaultScopeDbs {
    /// Nowy dostęp (`root` = `%LOCALAPPDATA%\Alfa\memory`).
    pub fn new(
        root: impl Into<PathBuf>,
        vault: Arc<dyn KeyVault>,
        sessions: Arc<dyn SessionDbProvider>,
    ) -> Self {
        Self {
            root: root.into(),
            vault,
            sessions,
            open: Mutex::new(BTreeMap::new()),
        }
    }

    /// Katalog baz.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<MemoryScope, Arc<Db>>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn owned(scope: &MemoryScope) -> Result<(String, String), MemoryError> {
        validate_scope(scope)?;
        match (scope_key_name(scope), scope_file_name(scope)) {
            (Some(key), Some(file)) => Ok((key, file)),
            _ => Err(MemoryError::invalid(
                "zakres sesji nie ma własnej bazy pamięci",
            )),
        }
    }
}

impl ScopeDbs for VaultScopeDbs {
    fn db(&self, scope: &MemoryScope, create: bool) -> Result<Option<Arc<Db>>, MemoryError> {
        if let MemoryScope::Session(s) = scope {
            return match self.sessions.session_db(s) {
                Ok(db) => Ok(Some(db)),
                Err(SessionError::NotFound { .. }) if !create => Ok(None),
                Err(e) => Err(MemoryError::storage(e)),
            };
        }
        let mut open = self.lock();
        if let Some(db) = open.get(scope) {
            return Ok(Some(Arc::clone(db)));
        }
        let (key_name, file) = Self::owned(scope)?;
        let path = self.root.join(file);
        let key = match self.vault.load(&key_name).map_err(MemoryError::storage)? {
            Some(key) => key,
            None if !create => return Ok(None),
            None => {
                remove_database(&path).map_err(MemoryError::storage)?;
                load_or_create_key(self.vault.as_ref(), &key_name).map_err(MemoryError::storage)?
            }
        };
        if !create && !path.exists() {
            return Ok(None);
        }
        let db = Arc::new(Db::open(&path, &key).map_err(MemoryError::storage)?);
        open.insert(scope.clone(), Arc::clone(&db));
        Ok(Some(db))
    }

    fn shred(&self, scope: &MemoryScope) -> Result<bool, MemoryError> {
        let (key_name, file) = Self::owned(scope)?;
        if let Some(db) = self.lock().remove(scope) {
            match Arc::try_unwrap(db) {
                Ok(db) => db.close().map_err(MemoryError::storage)?,
                Err(_) => {
                    return Err(MemoryError::conflict(
                        "baza zakresu jest w użyciu — spróbuj ponownie",
                    ));
                }
            }
        }
        let key = self.vault.delete(&key_name).map_err(MemoryError::storage)?;
        let files = remove_database(&self.root.join(file)).map_err(MemoryError::storage)?;
        Ok(key || !files.is_empty())
    }

    fn known(&self) -> Result<Vec<MemoryScope>, MemoryError> {
        let mut out: Vec<MemoryScope> = Vec::new();
        match std::fs::read_dir(&self.root) {
            Ok(dir) => {
                for entry in dir.flatten() {
                    if let Some(scope) = entry.file_name().to_str().and_then(scope_of_file) {
                        out.push(scope);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(MemoryError::storage(e)),
        }
        for s in self.sessions.session_ids().map_err(MemoryError::storage)? {
            out.push(MemoryScope::Session(s));
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_round_trip() {
        for scope in [
            MemoryScope::Global,
            MemoryScope::Project("dom-1".into()),
            MemoryScope::Agent(AgentId::new("beta")),
        ] {
            let file = scope_file_name(&scope).unwrap();
            assert_eq!(scope_of_file(&file), Some(scope.clone()));
            assert!(scope_key_name(&scope).unwrap().starts_with("alfa/memory/"));
        }
        assert_eq!(scope_of_file("project-..x.db"), None);
        assert_eq!(scope_of_file("global.db-wal"), None);
        assert_eq!(scope_of_file("inne.db"), None);
    }
}
