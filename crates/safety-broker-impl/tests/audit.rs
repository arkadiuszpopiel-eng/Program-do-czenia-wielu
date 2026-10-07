//! Audyt (ACC-F3-safety-broker-04): łańcuch weryfikowalny po 10 000 zdarzeń; modyfikacja,
//! usunięcie, wstawienie, ucięcie ogona i podmiana pliku — wykrywane.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use core_bus_contract::{Event, EventKind, Level};
use core_log_contract::{AuditWriter, RegexRedactor};
use safety_broker_contract::contract_tests::{delta, request, tree};
use safety_broker_contract::{AnchorStore, Broker, Capability, CommandOrigin, EVENT_CHAIN_STARTED};
use safety_broker_impl::audit::{
    AuditSink, BrokerAuditWriter, ChainError, FileAnchorStore, MemoryAnchorStore, verify_bytes,
};
use watchdog_contract::ManualClock;

fn open(
    path: &Path,
    anchor: Arc<dyn AnchorStore>,
    pred: Option<&str>,
) -> Result<BrokerAuditWriter, ChainError> {
    BrokerAuditWriter::open(
        path,
        anchor,
        Arc::new(RegexRedactor::default()),
        Arc::new(ManualClock::new(1_700_000_000_000)),
        "chain-test",
        pred,
    )
}

fn ev(i: u64) -> Event {
    Event::new(
        EventKind::Custom("test.audit".into()),
        Level::Audit,
        serde_json::json!({ "i": i, "ułamek": 0.1, "tekst": "zażółć" }),
    )
}

#[test]
fn chain_of_10k_events_verifies_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.ndjson");
    let anchor: Arc<dyn AnchorStore> = Arc::new(MemoryAnchorStore::default());
    let w = open(&path, anchor.clone(), Some("ab12")).unwrap();
    for i in 0..10_000 {
        w.record(&ev(i)).unwrap();
    }
    let s = w.verify_chain().unwrap();
    assert_eq!(s.records, 10_001);
    drop(w);
    let w = open(&path, anchor.clone(), None).unwrap();
    let r = w.record(&ev(10_000)).unwrap();
    assert_eq!(r.seq, 10_001);
    assert_eq!(w.verify_chain().unwrap().records, 10_002);
    let first = fs::read_to_string(&path).unwrap();
    let genesis = first.lines().next().unwrap();
    assert!(genesis.contains(EVENT_CHAIN_STARTED) && genesis.contains("ab12"));
}

fn tampered(path: &Path, f: impl FnOnce(&mut Vec<String>)) -> Vec<u8> {
    let text = fs::read_to_string(path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    f(&mut lines);
    let mut out = lines.join("\n").into_bytes();
    out.push(b'\n');
    out
}

#[test]
fn tampering_is_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.ndjson");
    let anchor: Arc<dyn AnchorStore> = Arc::new(MemoryAnchorStore::default());
    let w = open(&path, anchor.clone(), None).unwrap();
    for i in 0..20 {
        w.record(&ev(i)).unwrap();
    }
    drop(w);
    let modify = tampered(&path, |l| l[5] = l[5].replace("\"i\":4", "\"i\":5"));
    assert!(verify_bytes(&modify).is_err());
    let delete = tampered(&path, |l| {
        l.remove(7);
    });
    assert!(verify_bytes(&delete).is_err());
    let insert = tampered(&path, |l| {
        let dup = l[3].clone();
        l.insert(4, dup);
    });
    assert!(verify_bytes(&insert).is_err());
    let reorder = tampered(&path, |l| l.swap(2, 3));
    assert!(verify_bytes(&reorder).is_err());
    let spaced = tampered(&path, |l| l[1] = l[1].replacen(':', ": ", 1));
    assert!(matches!(
        verify_bytes(&spaced),
        Err(ChainError::NotCanonical { .. })
    ));
    // Ucięty ogon jest poprawnym łańcuchem — wykrywa go dopiero kotwica.
    let truncated = tampered(&path, |l| {
        l.truncate(10);
    });
    assert!(verify_bytes(&truncated).is_ok());
    fs::write(&path, &truncated).unwrap();
    assert!(matches!(
        open(&path, anchor.clone(), None),
        Err(ChainError::AnchorMismatch)
    ));
    // Podmiana całego pliku na inny poprawny łańcuch — także wykryta kotwicą.
    let other = dir.path().join("other.ndjson");
    let w2 = open(&other, Arc::new(MemoryAnchorStore::default()), None).unwrap();
    w2.record(&ev(1)).unwrap();
    drop(w2);
    fs::copy(&other, &path).unwrap();
    assert!(matches!(
        open(&path, anchor.clone(), None),
        Err(ChainError::AnchorMismatch)
    ));
    // Usunięcie pliku przy istniejącej kotwicy też jest naruszeniem.
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        open(&path, anchor, None),
        Err(ChainError::AnchorMismatch)
    ));
    // Urwany ostatni bajt.
    let mut bytes = fs::read(&other).unwrap();
    bytes.pop();
    assert_eq!(verify_bytes(&bytes), Err(ChainError::Truncated));
}

#[tokio::test]
async fn file_anchor_and_audit_writer_trait() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.ndjson");
    let anchor: Arc<dyn AnchorStore> =
        Arc::new(FileAnchorStore::new(dir.path().join("anchor.json")));
    let w = open(&path, anchor.clone(), None).unwrap();
    let r = w.append_audit(&ev(1)).await.unwrap();
    assert_eq!(w.head_hash().await.unwrap(), Some(r.hash.clone()));
    assert_eq!(anchor.load().unwrap().unwrap().head, r.hash);
    let mut secret = ev(2);
    secret.payload =
        serde_json::json!({ "klucz": "sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789" });
    w.record(&secret).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains("abcdefghijklmnopqrstuvwxyz0123456789"),
        "redakcja sekretów"
    );
    assert_eq!(w.path(), path.as_path());
}

#[tokio::test]
async fn broker_decisions_land_in_verifiable_chain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.ndjson");
    let writer = Arc::new(open(&path, Arc::new(MemoryAnchorStore::default()), None).unwrap());
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = common::engine_with(
        safety_broker_contract::contract_tests::test_policy(),
        clock,
        writer.clone(),
    );
    for i in 0..50 {
        let cap = Capability::FsRead(tree(&format!(r"C:\Users\ala\Docs\d{i}")));
        b.decide(request(&delta(), cap, CommandOrigin::UserText))
            .await
            .unwrap();
    }
    let creds = Capability::FsRead(tree(r"%USERPROFILE%\.claude"));
    b.decide(request(&delta(), creds, CommandOrigin::UserText))
        .await
        .unwrap();
    let summary = writer.verify_chain().unwrap();
    assert_eq!(summary.records, 52);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("broker.kernel_block") && text.contains("credential_denylist"));
}
