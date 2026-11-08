//! Testy atrapy: kontrakt współdzielony + rejestr intencji + limit migawki.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use artifacts_contract::contract_tests;
use artifacts_contract::{ArtifactAction, ArtifactError, Artifacts, Origin, SessionId};
use artifacts_fake::FakeArtifacts;

#[test]
fn contract_suite() {
    contract_tests::run_all(|| Box::new(FakeArtifacts::default()));
}

#[test]
fn intents_are_recorded_and_old_big_versions_are_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let a = FakeArtifacts::default().with_snapshot_max(4);
    let s = SessionId::new("s");
    let path = dir.path().join("duzy.txt");
    std::fs::write(&path, "pierwsza wersja").unwrap();
    let art = a.register(&s, &path, Origin::User, None).unwrap();
    assert!(!art.versions[0].snapshot);
    std::fs::write(&path, "druga, dłuższa wersja").unwrap();
    a.register(&s, &path, Origin::User, None).unwrap();
    assert!(matches!(
        a.preview(&s, &art.id, Some(1), 100),
        Err(ArtifactError::ContentUnavailable { .. })
    ));
    assert!(a.preview(&s, &art.id, None, 100).is_ok());
    a.intent(&s, &art.id, None, ArtifactAction::CopyAsFile)
        .unwrap();
    assert_eq!(a.intents().len(), 1);
    assert_eq!(a.intents()[0].version, 2);
}
