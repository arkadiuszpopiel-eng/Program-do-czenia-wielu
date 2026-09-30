//! Atrapa logów jądra (docs/PLAN.md §4.5, SPEC core-log „Fake”).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::Event;
use core_log_contract::{
    AuditRecordRef, AuditWriter, LogError, LogQuery, LogRecord, LogSink, LogStream, RecordRef,
    Redactor, RegexRedactor,
};

/// Wersja schematu zapisywana w rekordach atrapy (jak w implementacji).
const SCHEMA_VERSION: u32 = core_bus_contract::EVENT_SCHEMA_VERSION;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Default)]
struct SinkState {
    records: BTreeMap<LogStream, Vec<LogRecord>>,
    fail_next: Option<LogError>,
}

/// `LogSink` w pamięci.
pub struct FakeLogSink {
    state: Mutex<SinkState>,
    redactor: Arc<dyn Redactor>,
}

impl Default for FakeLogSink {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeLogSink {
    /// Atrapa z redaktorem domyślnym (`RegexRedactor::default()`).
    pub fn new() -> Self {
        Self::with_redactor(Arc::new(RegexRedactor::default()))
    }

    /// Atrapa z własnym redaktorem.
    pub fn with_redactor(redactor: Arc<dyn Redactor>) -> Self {
        Self {
            state: Mutex::new(SinkState::default()),
            redactor,
        }
    }

    /// Liczba rekordów strumienia.
    pub fn count(&self, stream: LogStream) -> usize {
        lock(&self.state).records.get(&stream).map_or(0, Vec::len)
    }

    /// Rekordy strumienia w kolejności zapisu.
    pub fn records(&self, stream: LogStream) -> Vec<LogRecord> {
        lock(&self.state)
            .records
            .get(&stream)
            .cloned()
            .unwrap_or_default()
    }

    /// Następny `append` zwróci ten błąd (jednorazowo).
    pub fn fail_next(&self, error: LogError) {
        lock(&self.state).fail_next = Some(error);
    }
}

#[async_trait]
impl LogSink for FakeLogSink {
    async fn append(&self, stream: LogStream, event: &Event) -> Result<RecordRef, LogError> {
        let mut clean = event.clone();
        self.redactor.redact_value(&mut clean.payload);
        let mut state = lock(&self.state);
        if let Some(err) = state.fail_next.take() {
            return Err(err);
        }
        let records = state.records.entry(stream).or_default();
        let reference = RecordRef {
            stream,
            seq: records.len() as u64,
        };
        records.push(LogRecord {
            reference: reference.clone(),
            event: clean,
            schema_version: SCHEMA_VERSION,
        });
        Ok(reference)
    }

    async fn query(&self, query: LogQuery) -> Result<Vec<LogRecord>, LogError> {
        let state = lock(&self.state);
        let mut out: Vec<LogRecord> = state
            .records
            .values()
            .flatten()
            .filter(|r| query.matches(&r.reference, &r.event))
            .cloned()
            .collect();
        out.sort_by(|a, b| {
            (a.event.ts, a.reference.stream, a.reference.seq).cmp(&(
                b.event.ts,
                b.reference.stream,
                b.reference.seq,
            ))
        });
        if let Some(limit) = query.limit {
            out.truncate(limit);
        }
        Ok(out)
    }
}

/// FNV-1a 64 — deterministyczny, NIEKRYPTOGRAFICZNY skrót atrapy.
fn fnv1a(parts: &[&[u8]]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("fnv1a:{hash:016x}")
}

/// `AuditWriter` w pamięci z łańcuchem skrótów (do testów modułów piszących Audyt).
pub struct FakeAuditWriter {
    chain: Mutex<Vec<(AuditRecordRef, Event)>>,
    redactor: Arc<dyn Redactor>,
}

impl Default for FakeAuditWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeAuditWriter {
    /// Pusty łańcuch z redaktorem domyślnym.
    pub fn new() -> Self {
        Self {
            chain: Mutex::new(Vec::new()),
            redactor: Arc::new(RegexRedactor::default()),
        }
    }

    /// Zapisane rekordy (zdarzenia z ustawionym `prev_hash`).
    pub fn records(&self) -> Vec<(AuditRecordRef, Event)> {
        lock(&self.chain).clone()
    }
}

#[async_trait]
impl AuditWriter for FakeAuditWriter {
    async fn append_audit(&self, event: &Event) -> Result<AuditRecordRef, LogError> {
        let mut clean = event.clone();
        self.redactor.redact_value(&mut clean.payload);
        let mut chain = lock(&self.chain);
        clean.prev_hash = chain.last().map(|(r, _)| r.hash.clone());
        let body = serde_json::to_vec(&clean).map_err(|e| LogError::Io(e.to_string()))?;
        let seq = chain.len() as u64;
        let prev = clean.prev_hash.clone().unwrap_or_default();
        let reference = AuditRecordRef {
            seq,
            hash: fnv1a(&[prev.as_bytes(), &seq.to_le_bytes(), &body]),
        };
        chain.push((reference.clone(), clean));
        Ok(reference)
    }

    async fn head_hash(&self) -> Result<Option<String>, LogError> {
        Ok(lock(&self.chain).last().map(|(r, _)| r.hash.clone()))
    }
}
