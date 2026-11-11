//! Szyfrowana obsada: odtworzenie ostatniej wersji, append-only i usunięte sesje.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use app_store::AppStore;
use lib_sqlstore::{Db, DbKey};
use personas_contract::{Cast, CastStore, Catalog, PersonaId};
use sessions_contract::{SessionDbProvider, SessionError, SessionId};

struct Provider {
    db: Arc<Db>,
    exists: AtomicBool,
}

impl SessionDbProvider for Provider {
    fn session_db(&self, _: &SessionId) -> Result<Arc<Db>, SessionError> {
        Ok(self.db.clone())
    }

    fn session_ids(&self) -> Result<Vec<SessionId>, SessionError> {
        Ok(if self.exists.load(Ordering::SeqCst) {
            vec![SessionId::new("s")]
        } else {
            vec![]
        })
    }
}

#[test]
fn encrypted_cast_reopens_with_latest_value_and_keeps_append_only_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.db");
    let key = DbKey::from_bytes([11; 32]);
    let session = SessionId::new("s");
    let catalog = Catalog::builtin();
    let before = catalog.default_cast(false);
    let after = Cast::solo(
        PersonaId::new("prywatna-persona-testowa"),
        catalog.roles().iter().map(|r| r.id.clone()),
        false,
    );
    {
        let db = Arc::new(Db::open(&path, &key).unwrap());
        let provider = Arc::new(Provider {
            db: db.clone(),
            exists: AtomicBool::new(true),
        });
        let store = AppStore::new(provider);
        store.save(&session, &before).unwrap();
        store.save(&session, &after).unwrap();
        db.with(|c| {
            assert_eq!(
                c.query_row("SELECT COUNT(*) FROM app_casts", [], |r| r.get::<_, i64>(0))?,
                2
            );
            assert!(c.execute("DELETE FROM app_casts", []).is_err());
            assert!(c.execute("UPDATE app_casts SET body='{}'", []).is_err());
            Ok::<(), lib_sqlstore::rusqlite::Error>(())
        })
        .unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        !bytes
            .windows(b"prywatna-persona-testowa".len())
            .any(|w| w == b"prywatna-persona-testowa")
    );
    let provider = Arc::new(Provider {
        db: Arc::new(Db::open(&path, &key).unwrap()),
        exists: AtomicBool::new(true),
    });
    let store = AppStore::new(provider.clone());
    assert_eq!(store.load().unwrap().get(&session), Some(&after));
    provider.exists.store(false, Ordering::SeqCst);
    assert!(
        store.load().unwrap().is_empty(),
        "usunięta sesja nie wraca z pamięci magazynu"
    );
}
