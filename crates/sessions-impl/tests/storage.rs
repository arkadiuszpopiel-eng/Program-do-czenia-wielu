//! Testy specyficzne dla SQLCipher: wyzwalacze append-only, szyfrowanie i testy szpiegowskie,
//! crypto-shredding, trwałość po ponownym otwarciu, sprzątanie sierot, indeksowanie atomowe.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use lib_sqlstore::DbKey;
use search_fake::RecordingIndexer;
use sessions_contract::{
    INDEX_KEY_NAME, KeyVault, NewSession, NewTurn, SessionCatalog, SessionDbProvider, SessionError,
    SessionHistory, SessionQuery, session_key_name,
};
use sessions_fake::fixtures::barge_in_conversation;
use sessions_impl::SqliteSessions;

const SECRET_A: &str = "sekret-sesji-A-zolty-kot-3141";

#[test]
fn raw_update_and_delete_of_turns_are_rejected_by_triggers() {
    let h = common::harness();
    let c = barge_in_conversation(&*h).unwrap();
    let db = h.session_db(&c.session).unwrap();
    db.with(|conn| {
        for sql in [
            "UPDATE turns SET body = x'00' WHERE id = 1",
            "UPDATE turns SET parent_id = NULL",
            "DELETE FROM turns WHERE id = 2",
            "UPDATE turn_heard SET chars = 1",
            "DELETE FROM turn_heard",
            "UPDATE branches SET base_turn_id = NULL",
            "DELETE FROM branches",
        ] {
            let err = conn.execute(sql, []).unwrap_err().to_string();
            assert!(err.contains("append-only"), "{sql}: {err}");
        }
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(h.turn(&c.session, c.a1.id).unwrap().content, c.a1.content);
}

#[test]
fn three_parallel_sessions_do_not_leak() {
    let h = common::harness();
    let ids: Vec<_> = (0..3)
        .map(|i| {
            h.create_session(NewSession {
                title: format!("S{i}"),
                ..NewSession::default()
            })
            .unwrap()
            .id
        })
        .collect();
    std::thread::scope(|scope| {
        for (i, id) in ids.iter().enumerate() {
            let s = &h.sessions;
            scope.spawn(move || {
                let mut parent = None;
                for n in 0..50 {
                    let t = s
                        .append_turn(id, parent, NewTurn::user(format!("znacznik-{i}-{n}")))
                        .unwrap();
                    parent = Some(t.id);
                }
            });
        }
    });
    for (i, id) in ids.iter().enumerate() {
        let leaf = h.active_leaf(id).unwrap().unwrap();
        let turns = h.branch_projection(id, leaf).unwrap();
        assert_eq!(turns.len(), 50);
        assert!(
            turns
                .iter()
                .all(|t| t.content.text.starts_with(&format!("znacznik-{i}-")))
        );
    }
}

#[test]
fn files_are_encrypted_and_keys_are_per_session() {
    let h = common::harness();
    let a = h.create_session(NewSession::default()).unwrap().id;
    let b = h.create_session(NewSession::default()).unwrap().id;
    h.append_turn(&a, None, NewTurn::user(SECRET_A)).unwrap();
    h.append_turn(&b, None, NewTurn::user("zwykła treść B"))
        .unwrap();
    let path_a = h.session_path(&a);
    let path_b = h.session_path(&b);
    h.close_session(&a);
    h.close_session(&b);
    assert!(
        !common::file_contains(&path_a, SECRET_A),
        "A jawnie na dysku"
    );
    assert!(!common::file_contains(&path_b, SECRET_A), "A w pliku B");
    let index = h.config().data_dir.join("index.db");
    assert!(!common::file_contains(&index, SECRET_A));
    let key_b = h.vault.load(&session_key_name(&b)).unwrap().unwrap();
    assert!(matches!(
        lib_sqlstore::open_connection(&path_a, &key_b),
        Err(lib_sqlstore::StoreError::NotDatabaseOrWrongKey)
    ));
    assert_ne!(key_b, h.vault.load(INDEX_KEY_NAME).unwrap().unwrap());
}

#[test]
fn delete_is_crypto_shredding() {
    let h = common::harness();
    let c = barge_in_conversation(&*h).unwrap();
    let path = h.session_path(&c.session);
    assert!(path.exists());
    let report = h.delete_session(&c.session).unwrap();
    assert!(report.key_deleted);
    assert!(report.files_removed >= 1);
    for file in lib_sqlstore::database_files(&path) {
        assert!(!file.exists(), "{} przetrwał", file.display());
    }
    assert_eq!(h.vault.load(&session_key_name(&c.session)).unwrap(), None);
    assert!(h.session_ids().unwrap().is_empty());
    assert!(matches!(
        h.session_db(&c.session),
        Err(SessionError::NotFound { .. })
    ));
}

#[test]
fn data_survives_reopen_and_orphans_are_swept() {
    let h = common::harness();
    let c = barge_in_conversation(&*h).unwrap();
    h.save_draft(&c.session, "szkic").unwrap();
    let orphan = h.config().data_dir.join("sierota.db");
    std::fs::write(&orphan, b"stary szyfrogram").unwrap();
    let config = h.config().clone();
    let vault = h.vault.clone();
    let common::Harness { dir, sessions, .. } = h;
    drop(sessions);
    let reopened = SqliteSessions::open(config, vault).unwrap();
    assert!(!orphan.exists());
    assert_eq!(reopened.active_leaf(&c.session).unwrap(), Some(c.a2.id));
    assert_eq!(
        reopened.draft(&c.session).unwrap().as_deref(),
        Some("szkic")
    );
    let proj = reopened.branch_projection(&c.session, c.a2.id).unwrap();
    assert_eq!(proj[1].heard_text(), Some("Dziś będzie słonecznie"));
    let list = reopened.list_sessions(&SessionQuery::default()).unwrap();
    assert_eq!((list[0].turns, list[0].unread), (5, 3));
    drop(dir);
}

#[test]
fn missing_key_is_reported_not_recreated() {
    let h = common::harness();
    let id = h.create_session(NewSession::default()).unwrap().id;
    h.close_session(&id);
    h.vault.delete(&session_key_name(&id)).unwrap();
    assert!(matches!(h.session_db(&id), Err(SessionError::Vault { .. })));
    h.vault
        .store(&session_key_name(&id), &DbKey::generate().unwrap())
        .unwrap();
    assert!(matches!(
        h.session_db(&id),
        Err(SessionError::Storage { .. })
    ));
}

#[test]
fn indexer_runs_in_the_append_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let indexer = Arc::new(RecordingIndexer::new());
    let vault = Arc::new(sessions_fake::MemoryKeyVault::new());
    let s = SqliteSessions::open(common::config(&dir), vault)
        .unwrap()
        .with_indexer(indexer.clone());
    let id = s.create_session(NewSession::default()).unwrap().id;
    let u = s
        .append_turn(&id, None, NewTurn::user("Żółć w tekście"))
        .unwrap();
    let docs = indexer.indexed();
    assert_eq!(docs.len(), 1);
    assert_eq!(
        (docs[0].id.key.as_str(), docs[0].text.as_str()),
        ("1", "Żółć w tekście")
    );
    indexer.set_failing(true);
    let failed = s.append_turn(&id, Some(u.id), NewTurn::assistant("alfa", "odpowiedź"));
    assert!(matches!(failed, Err(SessionError::Storage { .. })));
    assert_eq!(
        s.turn_count(&id).unwrap(),
        1,
        "tura wycofana razem z indeksem"
    );
    assert_eq!(s.active_leaf(&id).unwrap(), Some(u.id));
}
