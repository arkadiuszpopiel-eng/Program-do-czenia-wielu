//! Jednorazowy odczyt nie zatrzymuje baz w RAM i nie rozdziela czynnego połączenia.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use sessions_contract::{NewSession, SessionCatalog, SessionDbProvider};

#[test]
fn release_idle_keeps_borrowed_database_and_reopens_closed_one() {
    let h = common::harness();
    let session = h.create_session(NewSession::default()).unwrap();
    h.close_session(&session.id);
    assert_eq!(h.open_count(), 0);
    let first = h.session_db(&session.id).unwrap();
    h.release_idle(&session.id);
    assert!(
        h.is_open(&session.id),
        "czynny odbiorca utrzymuje połączenie"
    );
    let second = h.session_db(&session.id).unwrap();
    assert!(Arc::ptr_eq(&first, &second), "jedna instancja Db na plik");
    drop(first);
    drop(second);
    h.release_idle(&session.id);
    assert_eq!(h.open_count(), 0);
    assert_eq!(
        h.session(&session.id).unwrap(),
        session,
        "odczyt nie usuwa danych"
    );
    drop(h.session_db(&session.id).unwrap());
    assert_eq!(h.open_count(), 1);
    h.release_idle(&session.id);
    assert_eq!(h.open_count(), 0);
}
