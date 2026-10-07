//! Writer strumienia Audyt — Broker jest jedynym writerem (PLAN §8.1, ADR 3).
//!
//! Plik NDJSON; linia = kanoniczny JSON `{"chain", "event", "hash", "seq", "writer",
//! "written_at"}`, gdzie `event.prev_hash` = hash poprzedniego rekordu, a `hash` = SHA-256
//! kanonicznego JSON-u rekordu bez pola `hash`. Nowy łańcuch zaczyna rekord
//! `broker.audit.chain_started` z głową łańcucha `pre-broker` (przejęcie strumienia). Głowa
//! jest kotwiczona po każdym zapisie w [`AnchorStore`] (poza plikiem) — ucięcie ogona albo
//! podmiana pliku są wykrywane przy otwarciu i w [`BrokerAuditWriter::verify_chain`].

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::{Event, Level};
use core_log_contract::{AuditRecordRef, AuditWriter, LogError, Redactor};
use safety_broker_contract::{AnchorStore, ChainAnchor, EVENT_CHAIN_STARTED, event_kind};
use serde_json::{Value, json};
use watchdog_contract::Clock;

use super::AuditSink;
use super::canonical::{canonical_json, sha256_hex};

/// Oznaczenie writera w rekordach.
pub const BROKER_WRITER: &str = "safety-broker";
const KEYS: [&str; 6] = ["chain", "event", "hash", "seq", "writer", "written_at"];

/// Podsumowanie poprawnego łańcucha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainSummary {
    /// Identyfikator łańcucha (`None` dla pustego).
    pub chain_id: Option<String>,
    /// Liczba rekordów.
    pub records: u64,
    /// Hash głowy.
    pub head: Option<String>,
}

/// Naruszenie łańcucha (numer linii = `seq`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    /// Odczyt.
    #[error("odczyt łańcucha: {0}")]
    Io(String),
    /// Urwany ostatni rekord.
    #[error("łańcuch urwany (brak końcowego znaku nowej linii)")]
    Truncated,
    /// Rekord uszkodzony.
    #[error("linia {line}: rekord uszkodzony ({reason})")]
    Malformed {
        /// Linia.
        line: u64,
        /// Powód.
        reason: String,
    },
    /// Linia w postaci niekanonicznej.
    #[error("linia {line}: zapis niekanoniczny")]
    NotCanonical {
        /// Linia.
        line: u64,
    },
    /// Hash, `prev_hash`, `seq`, writer albo łańcuch niezgodne (modyfikacja/wstawienie/usunięcie).
    #[error("linia {line}: naruszona ciągłość łańcucha ({what})")]
    Broken {
        /// Linia.
        line: u64,
        /// Co się nie zgadza.
        what: &'static str,
    },
    /// Głowa pliku różni się od kotwicy (ucięty ogon, podmieniony plik).
    #[error("głowa łańcucha różni się od kotwicy")]
    AnchorMismatch,
}

fn malformed(line: u64, reason: impl Into<String>) -> ChainError {
    ChainError::Malformed {
        line,
        reason: reason.into(),
    }
}

fn verify_line(
    n: u64,
    raw: &[u8],
    prev: Option<&str>,
    chain: Option<&str>,
) -> Result<(String, String), ChainError> {
    let broken = |what| ChainError::Broken { line: n, what };
    let text = std::str::from_utf8(raw).map_err(|_| malformed(n, "UTF-8"))?;
    let mut value: Value = serde_json::from_str(text).map_err(|e| malformed(n, e.to_string()))?;
    if canonical_json(&value) != text {
        return Err(ChainError::NotCanonical { line: n });
    }
    let obj = value
        .as_object_mut()
        .ok_or_else(|| malformed(n, "nie obiekt"))?;
    if obj.len() != KEYS.len() || !KEYS.iter().all(|k| obj.contains_key(*k)) {
        return Err(malformed(n, "zestaw pól"));
    }
    let Some(Value::String(stored)) = obj.remove("hash") else {
        return Err(malformed(n, "hash"));
    };
    if obj.get("seq").and_then(Value::as_u64) != Some(n) {
        return Err(broken("seq"));
    }
    if obj.get("writer").and_then(Value::as_str) != Some(BROKER_WRITER) {
        return Err(broken("writer"));
    }
    let chain_id = obj
        .get("chain")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(n, "chain"))?
        .to_owned();
    if chain.is_some_and(|c| c != chain_id) {
        return Err(broken("chain"));
    }
    let event: Event = serde_json::from_value(obj.get("event").cloned().unwrap_or_default())
        .map_err(|e| malformed(n, format!("event: {e}")))?;
    if event.prev_hash.as_deref() != prev {
        return Err(broken("prev_hash"));
    }
    if sha256_hex(canonical_json(&value).as_bytes()) != stored {
        return Err(broken("hash"));
    }
    Ok((stored, chain_id))
}

/// Weryfikuje łańcuch w bajtach.
pub fn verify_bytes(bytes: &[u8]) -> Result<ChainSummary, ChainError> {
    let Some(body) = bytes.strip_suffix(b"\n") else {
        if bytes.is_empty() {
            return Ok(ChainSummary {
                chain_id: None,
                records: 0,
                head: None,
            });
        }
        return Err(ChainError::Truncated);
    };
    let mut head: Option<String> = None;
    let mut chain: Option<String> = None;
    let mut records = 0;
    for (n, raw) in (0u64..).zip(body.split(|b| *b == b'\n')) {
        let (hash, id) = verify_line(n, raw, head.as_deref(), chain.as_deref())?;
        head = Some(hash);
        chain = Some(id);
        records = n + 1;
    }
    Ok(ChainSummary {
        chain_id: chain,
        records,
        head,
    })
}

/// Weryfikuje plik (brak pliku = pusty łańcuch) i zgodność z kotwicą.
pub fn verify_file(path: &Path, anchor: Option<&ChainAnchor>) -> Result<ChainSummary, ChainError> {
    let summary = match fs::read(path) {
        Ok(bytes) => verify_bytes(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => verify_bytes(&[])?,
        Err(e) => return Err(ChainError::Io(e.to_string())),
    };
    let matches = match anchor {
        None => summary.records == 0,
        Some(a) => {
            summary.chain_id.as_deref() == Some(a.chain_id.as_str())
                && summary.records == a.records
                && summary.head.as_deref() == Some(a.head.as_str())
        }
    };
    if !matches {
        return Err(ChainError::AnchorMismatch);
    }
    Ok(summary)
}

struct State {
    chain_id: String,
    next_seq: u64,
    head: Option<String>,
    file: Option<File>,
    poisoned: bool,
}

/// Writer Audytu Brokera.
pub struct BrokerAuditWriter {
    path: PathBuf,
    anchor: Arc<dyn AnchorStore>,
    redactor: Arc<dyn Redactor>,
    clock: Arc<dyn Clock>,
    state: Mutex<State>,
}

impl BrokerAuditWriter {
    /// Otwiera łańcuch (weryfikacja pliku i kotwicy). Pusty → nowy łańcuch `chain_id` z rekordem
    /// startowym wskazującym głowę łańcucha `pre-broker` (`predecessor`).
    pub fn open(
        path: impl Into<PathBuf>,
        anchor: Arc<dyn AnchorStore>,
        redactor: Arc<dyn Redactor>,
        clock: Arc<dyn Clock>,
        chain_id: &str,
        predecessor: Option<&str>,
    ) -> Result<Self, ChainError> {
        let path = path.into();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| ChainError::Io(e.to_string()))?;
        }
        let stored = anchor.load().map_err(ChainError::Io)?;
        let summary = verify_file(&path, stored.as_ref())?;
        let fresh = summary.records == 0;
        let writer = Self {
            path,
            anchor,
            redactor,
            clock,
            state: Mutex::new(State {
                chain_id: summary.chain_id.unwrap_or_else(|| chain_id.to_owned()),
                next_seq: summary.records,
                head: summary.head,
                file: None,
                poisoned: false,
            }),
        };
        if fresh {
            let payload = json!({ "chain": chain_id, "predecessor_head": predecessor });
            let genesis = Event::new(event_kind(EVENT_CHAIN_STARTED), Level::Audit, payload);
            writer
                .append(&genesis)
                .map_err(|e| ChainError::Io(e.to_string()))?;
        }
        Ok(writer)
    }

    /// Ścieżka pliku.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Weryfikacja pliku względem kotwicy.
    pub fn verify_chain(&self) -> Result<ChainSummary, ChainError> {
        let anchor = self.anchor.load().map_err(ChainError::Io)?;
        verify_file(&self.path, anchor.as_ref())
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn append(&self, event: &Event) -> Result<AuditRecordRef, LogError> {
        let io = |what: &str, e: &dyn std::fmt::Display| LogError::Io(format!("{what}: {e}"));
        let mut clean = event.clone();
        self.redactor.redact_value(&mut clean.payload);
        let mut st = self.lock();
        if st.poisoned {
            return Err(LogError::Io(
                "Audyt po błędzie zapisu — wymagane ponowne otwarcie".into(),
            ));
        }
        clean.prev_hash = st.head.clone();
        let seq = st.next_seq;
        let written_at = chrono::DateTime::from_timestamp_millis(
            i64::try_from(self.clock.now_ms()).unwrap_or(i64::MAX),
        )
        .unwrap_or_default();
        let event_value = serde_json::to_value(&clean).map_err(|e| io("serializacja", &e))?;
        let mut record = json!({
            "chain": st.chain_id,
            "event": event_value,
            "seq": seq,
            "writer": BROKER_WRITER,
            "written_at": written_at,
        });
        let hash = sha256_hex(canonical_json(&record).as_bytes());
        if let Some(obj) = record.as_object_mut() {
            obj.insert("hash".into(), Value::String(hash.clone()));
        }
        let mut line = canonical_json(&record);
        line.push('\n');
        if st.file.is_none() {
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .map_err(|e| io("otwarcie Audytu", &e))?;
            st.file = Some(f);
        }
        let write = st
            .file
            .as_mut()
            .map(|f| f.write_all(line.as_bytes()).and_then(|()| f.flush()));
        if !matches!(write, Some(Ok(()))) {
            st.poisoned = true;
            return Err(LogError::Io("zapis Audytu nieudany".into()));
        }
        let anchor = ChainAnchor {
            chain_id: st.chain_id.clone(),
            records: seq + 1,
            head: hash.clone(),
        };
        if let Err(e) = self.anchor.store(&anchor) {
            st.poisoned = true;
            return Err(LogError::Io(format!("kotwica Audytu: {e}")));
        }
        st.next_seq += 1;
        st.head = Some(hash.clone());
        Ok(AuditRecordRef { seq, hash })
    }
}

impl AuditSink for BrokerAuditWriter {
    fn record(&self, event: &Event) -> Result<AuditRecordRef, LogError> {
        self.append(event)
    }
}

#[async_trait]
impl AuditWriter for BrokerAuditWriter {
    async fn append_audit(&self, event: &Event) -> Result<AuditRecordRef, LogError> {
        self.append(event)
    }

    async fn head_hash(&self) -> Result<Option<String>, LogError> {
        Ok(self.lock().head.clone())
    }
}
