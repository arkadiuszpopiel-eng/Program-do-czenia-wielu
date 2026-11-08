//! Testy atrapy logów: kontrakt współdzielony + liczniki, wstrzykiwanie błędów, łańcuch.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_bus_contract::EventKind;
use core_log_contract::contract_tests::{self, event_at};
use core_log_contract::{AuditWriter, LogError, LogSink, LogStream};
use core_log_fake::{FakeAuditWriter, FakeLogSink};
use serde_json::json;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(FakeLogSink::new).await;
    contract_tests::run_audit(FakeAuditWriter::new).await;
}

#[tokio::test]
async fn counts_and_injected_failure() {
    let sink = FakeLogSink::default();
    let ev = event_at(EventKind::Tool, 0, Some("s1"), json!("krok"));
    sink.append(LogStream::ToolsGui, &ev).await.unwrap();
    sink.fail_next(LogError::DiskLimit(LogStream::ToolsGui));
    assert_eq!(
        sink.append(LogStream::ToolsGui, &ev).await,
        Err(LogError::DiskLimit(LogStream::ToolsGui))
    );
    let r = sink.append(LogStream::ToolsGui, &ev).await.unwrap();
    assert_eq!(r.seq, 1, "nieudany zapis nie zużywa numeru");
    assert_eq!(sink.count(LogStream::ToolsGui), 2);
    assert_eq!(sink.count(LogStream::Voice), 0);
    assert_eq!(
        sink.records(LogStream::ToolsGui)[0].event.payload,
        json!("krok")
    );
}

#[tokio::test]
async fn audit_chain_links_prev_hash_and_redacts() {
    let audit = FakeAuditWriter::default();
    let ev = event_at(
        EventKind::Audit,
        0,
        None,
        json!({"k": "sk-ant-api03-abcdefghijklmnop"}),
    );
    let a = audit.append_audit(&ev).await.unwrap();
    let b = audit.append_audit(&ev).await.unwrap();
    let records = audit.records();
    assert_eq!(records[0].1.prev_hash, None);
    assert_eq!(records[1].1.prev_hash, Some(a.hash.clone()));
    assert_eq!(audit.head_hash().await.unwrap(), Some(b.hash));
    assert!(!records[0].1.payload.to_string().contains("abcdefgh"));
    assert!(a.hash.starts_with("fnv1a:"));
}
