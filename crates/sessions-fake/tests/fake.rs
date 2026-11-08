//! Testy atrapy: kontrakt współdzielony, property-based append-only, fixture'y, sejf, dostawca baz.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use proptest::prelude::*;
use sessions_contract::contract_tests::{self, ops};
use sessions_contract::{
    SessionCatalog, SessionDbProvider, SessionHistory, SessionId, SessionQuery, session_key_name,
};
use sessions_fake::fixtures::{HEARD_CHARS, barge_in_conversation, linear_conversation};
use sessions_fake::{FakeSessions, MemoryKeyVault, TempDbProvider};

#[test]
fn contract_suite() {
    contract_tests::run_all(|| Box::new(FakeSessions::new()));
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn any_ops_keep_turns_unchanged(ops in proptest::collection::vec(ops::op_strategy(), 1..40)) {
        let s = FakeSessions::new();
        let (first, second) = ops.split_at(ops.len() / 2);
        let id = ops::check_ops(&s, first);
        ops::check_ops_in(&s, &id, second);
    }
}

#[test]
fn fixture_barge_in_has_branches_and_heard_prefix() {
    let s = FakeSessions::new();
    let c = barge_in_conversation(&s).unwrap();
    let a1 = s.turn(&c.session, c.a1.id).unwrap();
    assert_eq!(a1.heard_prefix.map(|p| p.chars), Some(HEARD_CHARS));
    assert_eq!(a1.heard_text(), Some("Dziś będzie słonecznie"));
    assert_eq!(s.siblings(&c.session, c.a1b.id).unwrap().turns.len(), 2);
    assert_eq!(s.active_leaf(&c.session).unwrap(), Some(c.a2.id));
    let proj = s.branch_projection(&c.session, c.a2.id).unwrap();
    assert_eq!(proj.len(), 4);
    let (_, turns) = linear_conversation(&s, 10).unwrap();
    assert_eq!(turns.len(), 10);
}

#[test]
fn deterministic_ids_and_clock() {
    let a = FakeSessions::new();
    let b = FakeSessions::new();
    let ma = barge_in_conversation(&a).unwrap();
    let mb = barge_in_conversation(&b).unwrap();
    assert_eq!(ma.session, mb.session);
    assert_eq!(ma.a2, mb.a2);
    assert_eq!(ma.session, SessionId::new("sess-0001"));
}

#[test]
fn delete_removes_key_from_vault() {
    let vault = Arc::new(MemoryKeyVault::new());
    let s = FakeSessions::with_vault(vault.clone());
    let c = barge_in_conversation(&s).unwrap();
    assert_eq!(vault.names(), vec![session_key_name(&c.session)]);
    let report = s.delete_session(&c.session).unwrap();
    assert!(report.key_deleted);
    assert!(vault.names().is_empty());
    assert!(
        s.list_sessions(&SessionQuery::default())
            .unwrap()
            .is_empty()
    );
    vault.set_unavailable(true);
    assert!(s.create_session(Default::default()).is_err());
}

#[test]
fn temp_db_provider_gives_same_db_and_drops_file() {
    let p = TempDbProvider::new().unwrap();
    let id = SessionId::new("s1");
    let db1 = p.session_db(&id).unwrap();
    let db2 = p.session_db(&id).unwrap();
    assert!(Arc::ptr_eq(&db1, &db2));
    assert_eq!(p.session_ids().unwrap(), vec![id.clone()]);
    let path = db1.path().to_path_buf();
    drop((db1, db2));
    p.drop_session(&id).unwrap();
    assert!(!path.exists());
    assert!(p.session_ids().unwrap().is_empty());
}
