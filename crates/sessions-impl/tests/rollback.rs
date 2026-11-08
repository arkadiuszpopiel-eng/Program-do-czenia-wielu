//! Fala 5, m-23 / ADR 0007: powrót do starszej wersji Alfy po migracji bazy sesji wykonanej przez
//! nowszą. Sesje zostają czytelne (dotąd `UnknownMigration` — sesja nie otwierała się wcale),
//! zapis jest odrzucany czytelnym błędem i niczego nie zmienia; migracja oznaczona przez nowszą
//! wersję jako addytywna pozwala pracować normalnie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use lib_sqlstore::rusqlite::Connection;
use lib_sqlstore::{migrate, migrate_with};
use search_contract::{Doc, DocId, RemoveReport, SearchError, SessionId, TxIndexer};
use sessions_contract::{NewTurn, SessionDbProvider, SessionError, SessionHistory};
use sessions_fake::fixtures::barge_in_conversation;
use sessions_impl::{SESSION_MIGRATIONS, SqliteSessions};

/// Migracje „nowszej wersji Alfy”: bieżące + `sessions` 0002 (nowa tabela).
fn newer() -> Vec<(&'static str, &'static str)> {
    let mut steps = SESSION_MIGRATIONS.to_vec();
    steps.push((
        "0002",
        "CREATE TABLE turn_reactions(turn_id INTEGER PRIMARY KEY, emoji TEXT NOT NULL);",
    ));
    steps
}

#[test]
fn sessions_stay_readable_after_rollback_and_writes_are_refused() {
    let h = common::harness();
    let c = barge_in_conversation(&*h).unwrap();
    let before = h.branch_projection(&c.session, c.a2.id).unwrap();
    h.session_db(&c.session)
        .unwrap()
        .with(|conn| migrate(conn, "sessions", &newer()).map(|_| ()))
        .unwrap();
    h.close_session(&c.session);
    // Ten kod = starsza wersja (zna tylko `sessions` 0001).
    assert_eq!(h.branch_projection(&c.session, c.a2.id).unwrap(), before);
    assert_eq!(h.turn(&c.session, c.a1.id).unwrap().content, c.a1.content);
    let err = h
        .append_turn(&c.session, Some(c.a2.id), NewTurn::user("Dalej?"))
        .unwrap_err();
    assert!(
        matches!(&err, SessionError::Storage { reason } if reason.contains("tylko do odczytu")),
        "{err:?}"
    );
    assert_eq!(
        h.all_turns(&c.session).unwrap().len(),
        5,
        "nic nie dopisano"
    );
    h.close_session(&c.session);
    assert_eq!(h.all_turns(&c.session).unwrap().len(), 5);
}

#[test]
fn additive_newer_migration_keeps_sessions_writable_after_rollback() {
    let h = common::harness();
    let c = barge_in_conversation(&*h).unwrap();
    h.session_db(&c.session)
        .unwrap()
        .with(|conn| migrate_with(conn, "sessions", &newer(), &["0002"]).map(|_| ()))
        .unwrap();
    h.close_session(&c.session);
    let t = h
        .append_turn(&c.session, Some(c.a2.id), NewTurn::user("Dalej?"))
        .unwrap();
    assert_eq!(h.turn(&c.session, t.id).unwrap().parent, Some(c.a2.id));
}

/// Indekser, którego `prepare` pisze do bazy (jak `search-impl` w trakcie przebudowy wektorów).
struct WritingIndexer;

impl TxIndexer for WritingIndexer {
    fn prepare(&self, conn: &Connection) -> Result<(), SearchError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS idx_meta(k TEXT PRIMARY KEY, v TEXT);
             INSERT OR REPLACE INTO idx_meta VALUES ('opened', datetime('now'));",
        )
        .map_err(SearchError::storage)
    }
    fn index_in(&self, _conn: &Connection, _doc: &Doc) -> Result<(), SearchError> {
        Ok(())
    }
    fn remove_in(
        &self,
        _conn: &Connection,
        _session: &SessionId,
        _id: &DocId,
    ) -> Result<RemoveReport, SearchError> {
        Ok(RemoveReport::default())
    }
}

#[test]
fn read_only_session_opens_even_when_the_index_cannot_prepare() {
    let h = common::harness();
    let c = barge_in_conversation(&*h).unwrap();
    h.session_db(&c.session)
        .unwrap()
        .with(|conn| migrate(conn, "sessions", &newer()).map(|_| ()))
        .unwrap();
    let id = c.session.clone();
    let common::Harness {
        dir,
        vault,
        sessions,
    } = h;
    drop(sessions);
    let reopened = SqliteSessions::open(common::config(&dir), vault)
        .unwrap()
        .with_indexer(Arc::new(WritingIndexer));
    assert_eq!(
        reopened.all_turns(&id).unwrap().len(),
        5,
        "historia czytelna"
    );
}
