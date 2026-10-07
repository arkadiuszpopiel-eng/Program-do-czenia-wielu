//! Współdzielone testy kontraktowe `LogSink`/`AuditWriter` na implementacji plikowej.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::Event;
use core_log_contract::{
    AuditRecordRef, AuditWriter, LogError, LogQuery, LogRecord, LogSink, LogStream, RecordRef,
    RegexRedactor, contract_tests,
};
use core_log_impl::{FileLogSink, LogOptions, PreBrokerAuditWriter, SystemClock};
use tempfile::TempDir;

/// Sink razem z katalogiem tymczasowym (katalog żyje tak długo jak sink).
struct TempSink {
    inner: FileLogSink,
    _dir: TempDir,
}

#[async_trait]
impl LogSink for TempSink {
    async fn append(&self, stream: LogStream, event: &Event) -> Result<RecordRef, LogError> {
        self.inner.append(stream, event).await
    }
    async fn query(&self, query: LogQuery) -> Result<Vec<LogRecord>, LogError> {
        self.inner.query(query).await
    }
}

struct TempAudit {
    inner: PreBrokerAuditWriter,
    _dir: TempDir,
}

#[async_trait]
impl AuditWriter for TempAudit {
    async fn append_audit(&self, event: &Event) -> Result<AuditRecordRef, LogError> {
        self.inner.append_audit(event).await
    }
    async fn head_hash(&self) -> Result<Option<String>, LogError> {
        self.inner.head_hash().await
    }
}

fn sink() -> TempSink {
    let dir = tempfile::tempdir().unwrap();
    TempSink {
        inner: FileLogSink::open(LogOptions::new(dir.path())).unwrap(),
        _dir: dir,
    }
}

fn audit() -> TempAudit {
    let dir = tempfile::tempdir().unwrap();
    let writer = PreBrokerAuditWriter::open(
        dir.path().join("audit/pre-broker.ndjson"),
        Arc::new(RegexRedactor::default()),
        Arc::new(SystemClock),
    )
    .unwrap();
    TempAudit {
        inner: writer,
        _dir: dir,
    }
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(sink).await;
    contract_tests::run_audit(audit).await;
}
