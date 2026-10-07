//! Testy `FileLogSink`: rotacja, limit dysku, retencja, odtwarzanie, redakcja (test szpiegowski),
//! manifest i log-writer podpięty do magistrali.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use core_bus_contract::{Event, EventBus, EventKind, Level};
use core_bus_fake::{FakeBus, VirtualClock};
use core_log_contract::{
    AuditWriter, LogError, LogQuery, LogSink, LogStream, RegexRedactor, contract_tests::event_at,
};
use core_log_impl::{
    FileLogSink, LogOptions, MODULE_TOML, PreBrokerAuditWriter, StreamLimits, spawn_bus_writer,
    stream_dir_name,
};
use serde_json::json;

fn open(root: &Path, options: impl FnOnce(&mut LogOptions), clock: &VirtualClock) -> FileLogSink {
    let mut o = LogOptions::new(root);
    options(&mut o);
    let c = clock.clone();
    FileLogSink::open_with(
        o,
        Arc::new(RegexRedactor::default()),
        Arc::new(move || c.now()),
    )
    .unwrap()
}

fn query(stream: LogStream) -> LogQuery {
    LogQuery {
        stream: Some(stream),
        ..LogQuery::default()
    }
}

fn seqs(records: &[core_log_contract::LogRecord]) -> Vec<u64> {
    records.iter().map(|r| r.reference.seq).collect()
}

#[test]
fn module_toml_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "core-log");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.provides[0].to_string(), "core-log-contract@1");
}

#[tokio::test]
async fn rotates_by_size_and_queries_across_segments() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::default();
    let sink = open(dir.path(), |o| o.max_segment_bytes = 600, &clock);
    for i in 0..20u64 {
        let ev = event_at(EventKind::Voice, i as i64, None, json!({"i": i}));
        sink.append(LogStream::Voice, &ev).await.unwrap();
    }
    assert!(sink.segment_count(LogStream::Voice) > 3);
    let all = sink.query(query(LogStream::Voice)).await.unwrap();
    assert_eq!(seqs(&all), (0..20).collect::<Vec<_>>());
    let from = LogQuery {
        from_seq: Some(17),
        ..query(LogStream::Voice)
    };
    assert_eq!(seqs(&sink.query(from).await.unwrap()), [17, 18, 19]);
}

#[tokio::test]
async fn disk_limit_drops_oldest_segments_and_rejects_huge_records() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::default();
    let limit = StreamLimits {
        disk_limit_bytes: 2_000,
        retention_days: None,
    };
    let sink = open(
        dir.path(),
        |o| {
            o.max_segment_bytes = 500;
            o.limits.insert(LogStream::Diagnostics, limit);
        },
        &clock,
    );
    for i in 0..40u64 {
        let ev = event_at(EventKind::Diagnostics, i as i64, None, json!(i));
        sink.append(LogStream::Diagnostics, &ev).await.unwrap();
    }
    assert!(sink.disk_usage(LogStream::Diagnostics) <= 2_000);
    let kept = seqs(&sink.query(query(LogStream::Diagnostics)).await.unwrap());
    assert!(kept[0] > 0, "najstarsze rekordy usunięte");
    assert_eq!(*kept.last().unwrap(), 39);
    let huge = event_at(EventKind::Diagnostics, 0, None, json!("x".repeat(3_000)));
    assert_eq!(
        sink.append(LogStream::Diagnostics, &huge).await,
        Err(LogError::DiskLimit(LogStream::Diagnostics))
    );
}

#[tokio::test]
async fn retention_removes_old_tools_gui_and_keeps_numbering() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::default();
    let sink = open(dir.path(), |_| {}, &clock);
    let gui = event_at(EventKind::Gui, 0, None, json!("zrzut"));
    let voice = event_at(EventKind::Voice, 0, None, json!("głos"));
    sink.append(LogStream::ToolsGui, &gui).await.unwrap();
    sink.append(LogStream::Voice, &voice).await.unwrap();
    clock.advance(chrono::Duration::days(6));
    assert_eq!(sink.enforce_retention().unwrap(), 0);
    clock.advance(chrono::Duration::days(2));
    assert_eq!(sink.enforce_retention().unwrap(), 1);
    assert!(
        sink.query(query(LogStream::ToolsGui))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(sink.query(query(LogStream::Voice)).await.unwrap().len(), 1);
    let next = sink.append(LogStream::ToolsGui, &gui).await.unwrap();
    assert_eq!(next.seq, 1, "numeracja nie wraca do zera");
    drop(sink);
    let reopened = open(dir.path(), |_| {}, &clock);
    assert_eq!(
        reopened
            .append(LogStream::ToolsGui, &gui)
            .await
            .unwrap()
            .seq,
        2
    );
}

#[tokio::test]
async fn reopen_continues_and_broken_tail_starts_new_segment() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::default();
    let sink = open(dir.path(), |_| {}, &clock);
    for i in 0..3 {
        let ev = event_at(EventKind::ModelCall, i, None, json!(i));
        sink.append(LogStream::ModelCalls, &ev).await.unwrap();
    }
    drop(sink);
    let stream_dir = dir.path().join(stream_dir_name(LogStream::ModelCalls));
    let segment = std::fs::read_dir(&stream_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&segment)
        .unwrap();
    f.write_all(b"{\"seq\":3,\"urwany").unwrap();
    drop(f);
    let sink = open(dir.path(), |_| {}, &clock);
    let ev = event_at(EventKind::ModelCall, 9, None, json!(9));
    assert_eq!(
        sink.append(LogStream::ModelCalls, &ev).await.unwrap().seq,
        3
    );
    assert_eq!(sink.segment_count(LogStream::ModelCalls), 2);
    let all = sink.query(query(LogStream::ModelCalls)).await.unwrap();
    assert_eq!(seqs(&all), [0, 1, 2, 3]);
}

/// Test szpiegowski (ACC-F1-core-log-04): żaden sekret z fixture'ów nie trafia do plików.
#[tokio::test]
async fn no_secret_reaches_disk() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::default();
    let sink = open(dir.path(), |_| {}, &clock);
    let secrets = [
        "sk-ant-api03-SEKRETabcdefgh123",
        "xai-SEKRETabcdefgh123",
        "AIzaSEKRETabcdefghijklmnopqrstu",
        "ghp_SEKRETabcdefghijklmnopqrstuvw",
    ];
    for (i, s) in secrets.iter().enumerate() {
        let payload = json!({"prompt": format!("użyj {s}"), "headers": [format!("Bearer {s}")]});
        let ev = event_at(EventKind::ModelCall, i as i64, None, payload);
        sink.append(LogStream::ModelCalls, &ev).await.unwrap();
    }
    let audit_path = dir.path().join("audit/pre-broker.ndjson");
    let audit = PreBrokerAuditWriter::open(
        &audit_path,
        Arc::new(RegexRedactor::default()),
        Arc::new(core_log_impl::SystemClock),
    )
    .unwrap();
    let ev = event_at(EventKind::Audit, 0, None, json!({"key": secrets[0]}));
    audit.append_audit(&ev).await.unwrap();
    let mut files = vec![audit_path];
    let stream_dir = dir.path().join(stream_dir_name(LogStream::ModelCalls));
    for entry in std::fs::read_dir(stream_dir).unwrap() {
        files.push(entry.unwrap().path());
    }
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.contains("SEKRET"), "sekret w {}", file.display());
    }
}

#[tokio::test]
async fn bus_writer_routes_events() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::default();
    let sink = Arc::new(open(dir.path(), |_| {}, &clock));
    let audit = Arc::new(
        PreBrokerAuditWriter::open(
            dir.path().join("audit/pre-broker.ndjson"),
            Arc::new(RegexRedactor::default()),
            Arc::new(core_log_impl::SystemClock),
        )
        .unwrap(),
    );
    let bus = Arc::new(FakeBus::new(clock.clone()));
    let handle = spawn_bus_writer(bus.clone(), sink.clone(), Some(audit.clone()))
        .await
        .unwrap();
    let kinds = [
        EventKind::Tool,
        EventKind::Audit,
        EventKind::Custom("registry.module.health".into()),
        EventKind::Ui,
    ];
    for kind in kinds {
        bus.publish(Event::new(kind, Level::Info, json!(1)))
            .await
            .unwrap();
    }
    for _ in 0..50 {
        if audit.head_hash().await.unwrap().is_some()
            && sink.query(LogQuery::default()).await.unwrap().len() == 2
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let all = sink.query(LogQuery::default()).await.unwrap();
    let streams: Vec<_> = all.iter().map(|r| r.reference.stream).collect();
    assert_eq!(streams, [LogStream::ToolsGui, LogStream::Diagnostics]);
    assert_eq!(audit.verify_chain().unwrap().records, 1);
    handle.abort();
}

#[tokio::test]
async fn torn_segment_without_records_is_quarantined() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::default();
    let stream_dir = dir.path().join(stream_dir_name(LogStream::Voice));
    std::fs::create_dir_all(&stream_dir).unwrap();
    let torn = stream_dir.join("00000000000000000000.ndjson");
    std::fs::write(&torn, b"{\"seq\":0,\"urw").unwrap();
    let sink = open(dir.path(), |_| {}, &clock);
    let ev = event_at(EventKind::Voice, 0, None, json!(1));
    assert_eq!(sink.append(LogStream::Voice, &ev).await.unwrap().seq, 0);
    assert!(
        stream_dir
            .join("00000000000000000000.ndjson.broken")
            .exists()
    );
    assert_eq!(sink.query(query(LogStream::Voice)).await.unwrap().len(), 1);
}
