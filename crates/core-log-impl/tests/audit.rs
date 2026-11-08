//! Łańcuch audytu pre-broker: trwałość, wykrywanie naruszeń, testy własności.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use core_bus_contract::{Event, EventKind, Level};
use core_log_contract::{AuditWriter, RegexRedactor};
use core_log_impl::{
    AuditOpenError, ChainError, PRE_BROKER_WRITER, PreBrokerAuditWriter, verify_against,
    verify_bytes, verify_file,
};
use proptest::prelude::*;
use serde_json::{Value, json};

fn writer(path: &Path) -> PreBrokerAuditWriter {
    let fixed = || chrono::DateTime::<chrono::Utc>::from_timestamp(1_767_225_600, 0).unwrap();
    PreBrokerAuditWriter::open(path, Arc::new(RegexRedactor::default()), Arc::new(fixed)).unwrap()
}

fn event(payload: Value) -> Event {
    Event::new(EventKind::Audit, Level::Audit, payload)
}

async fn chain_of(n: usize) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pre-broker.ndjson");
    let w = writer(&path);
    for i in 0..n {
        w.append_audit(&event(json!({"i": i}))).await.unwrap();
    }
    (dir, path)
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn write_lines(path: &Path, lines: &[String]) {
    std::fs::write(
        path,
        lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
    )
    .unwrap();
}

#[tokio::test]
async fn chain_survives_reopen_and_links_prev_hash() {
    let (_dir, path) = chain_of(2).await;
    let w = writer(&path);
    let head = w.head_hash().await.unwrap();
    let third = w.append_audit(&event(json!("x"))).await.unwrap();
    assert_eq!(third.seq, 2);
    let summary = w.verify_chain().unwrap();
    assert_eq!(summary.records, 3);
    assert_eq!(summary.head, Some(third.hash));
    let last: Value = serde_json::from_str(&lines(&path)[2]).unwrap();
    assert_eq!(last["writer"], PRE_BROKER_WRITER);
    assert_eq!(last["event"]["prev_hash"].as_str(), head.as_deref());
    let first: Value = serde_json::from_str(&lines(&path)[0]).unwrap();
    assert!(
        first["event"].get("prev_hash").is_none(),
        "pierwszy rekord bez poprzednika"
    );
}

#[tokio::test]
async fn modification_deletion_insertion_are_detected() {
    let (_dir, path) = chain_of(4).await;
    let original = lines(&path);

    let mut modified = original.clone();
    modified[1] = modified[1].replace("{\"i\":1}", "{\"i\":7}");
    write_lines(&path, &modified);
    assert_eq!(
        verify_file(&path),
        Err(ChainError::HashMismatch { line: 1 })
    );
    let broken = PreBrokerAuditWriter::open(
        &path,
        Arc::new(RegexRedactor::default()),
        Arc::new(core_log_impl::SystemClock),
    );
    assert!(matches!(broken, Err(AuditOpenError::Broken(_))));

    let mut deleted = original.clone();
    deleted.remove(1);
    write_lines(&path, &deleted);
    assert!(matches!(
        verify_file(&path),
        Err(ChainError::SeqMismatch { line: 1, found: 2 })
    ));

    let mut inserted = original.clone();
    inserted.insert(2, original[1].clone());
    write_lines(&path, &inserted);
    assert!(matches!(
        verify_file(&path),
        Err(ChainError::SeqMismatch { .. })
    ));

    let mut spaced = original.clone();
    spaced[0] = spaced[0].replacen(':', ": ", 1);
    write_lines(&path, &spaced);
    assert_eq!(
        verify_file(&path),
        Err(ChainError::NotCanonical { line: 0 })
    );
}

#[tokio::test]
async fn truncated_tail_is_detected_against_head() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pre-broker.ndjson");
    let w = writer(&path);
    for i in 0..3 {
        w.append_audit(&event(json!(i))).await.unwrap();
    }
    let mut kept = lines(&path);
    kept.pop();
    write_lines(&path, &kept);
    assert!(
        verify_file(&path).is_ok(),
        "sam plik jest spójnym, krótszym łańcuchem"
    );
    assert_eq!(w.verify_chain(), Err(ChainError::HeadMismatch));
    std::fs::write(&path, b"").unwrap();
    assert_eq!(verify_bytes(b"{}"), Err(ChainError::Truncated));
    assert_eq!(verify_file(&dir.path().join("brak")).unwrap().records, 0);
}

fn payload() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        any::<String>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        any::<f64>()
            .prop_filter("skończona", |f| f.is_finite())
            .prop_map(Value::from),
        any::<bool>().prop_map(Value::from),
    ];
    proptest::collection::vec(leaf, 0..4).prop_map(Value::Array)
}

fn build_chain(payloads: &[Value]) -> (tempfile::TempDir, PathBuf, Option<String>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pre-broker.ndjson");
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let head = rt.block_on(async {
        let w = writer(&path);
        for p in payloads {
            w.append_audit(&event(p.clone())).await.unwrap();
        }
        w.head_hash().await.unwrap()
    });
    (dir, path, head)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn any_chain_verifies_and_any_byte_flip_is_detected(
        payloads in proptest::collection::vec(payload(), 1..8),
        at in any::<prop::sample::Index>(),
        mask in 1u8..=255,
    ) {
        let (_dir, path, head) = build_chain(&payloads);
        let summary = verify_against(&path, head.as_deref()).unwrap();
        prop_assert_eq!(summary.records, payloads.len() as u64);
        let mut bytes = std::fs::read(&path).unwrap();
        let i = at.index(bytes.len());
        bytes[i] ^= mask;
        prop_assert!(verify_bytes(&bytes).is_err(), "nie wykryto zmiany bajtu {}", i);
    }

    #[test]
    fn removing_any_record_is_detected(
        payloads in proptest::collection::vec(payload(), 1..8),
        at in any::<prop::sample::Index>(),
    ) {
        let (_dir, path, head) = build_chain(&payloads);
        let mut all = lines(&path);
        all.remove(at.index(all.len()));
        write_lines(&path, &all);
        prop_assert!(verify_against(&path, head.as_deref()).is_err());
    }
}
