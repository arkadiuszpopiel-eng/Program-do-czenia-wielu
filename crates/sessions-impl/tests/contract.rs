//! Kontrakt współdzielony + property-based append-only (API i bajty kolumny `body`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use lib_sqlstore::rusqlite::Connection;
use proptest::prelude::*;
use sessions_contract::SessionDbProvider;
use sessions_contract::contract_tests::{self, ops};

#[test]
fn contract_suite() {
    contract_tests::run_all(common::harness);
}

fn raw_bodies(conn: &Connection) -> Vec<(i64, Vec<u8>)> {
    let mut stmt = conn
        .prepare("SELECT id, body FROM turns ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, failure_persistence: None, ..ProptestConfig::default() })]

    /// `ACC-F1-sessions-02`: dowolna sekwencja operacji → spójne drzewo, wcześniejsze tury
    /// bajtowo niezmienione (API) i surowe bajty `body` w bazie niezmienione.
    #[test]
    fn any_ops_keep_turns_and_raw_bytes(
        first in proptest::collection::vec(ops::op_strategy(), 1..20),
        second in proptest::collection::vec(ops::op_strategy(), 1..20),
    ) {
        let h = common::harness();
        let id = ops::check_ops(&*h, &first);
        let db = h.session_db(&id).unwrap();
        let before = db.with(|c| Ok::<_, ()>(raw_bodies(c))).unwrap();
        drop(db);
        ops::check_ops_in(&*h, &id, &second);
        let db = h.session_db(&id).unwrap();
        let after = db.with(|c| Ok::<_, ()>(raw_bodies(c))).unwrap();
        prop_assert!(after.len() >= before.len());
        prop_assert_eq!(&after[..before.len()], &before[..]);
    }
}
