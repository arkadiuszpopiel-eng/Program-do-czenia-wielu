//! Tymczasowy writer Audytu „pre-broker” (ARCHITECTURE §9: do F3 Audyt pisze `core-log`).
//!
//! Plik NDJSON; każda linia to kanoniczny JSON rekordu
//! `{"event", "hash", "seq", "writer": "pre-broker", "written_at"}`, gdzie `event.prev_hash`
//! = hash poprzedniego rekordu (brak w pierwszym), a `hash` = SHA-256 z kanonicznego JSON-u
//! rekordu bez pola `hash`. `verify_bytes` wykrywa modyfikację, usunięcie i wstawienie
//! rekordów; ucięcie ogona wykrywa porównanie z głową (`verify_chain`, `verify_against`).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::Event;
use core_log_contract::{AuditRecordRef, AuditWriter, LogError, Redactor};
use serde_json::{Value, json};

use crate::canonical::{canonical_json, sha256_hex};
use crate::options::Clock;
use crate::segment::io_err;

/// Oznaczenie writera w rekordach (Broker w F3 zaczyna nowy łańcuch z własnym oznaczeniem).
pub const PRE_BROKER_WRITER: &str = "pre-broker";

const KEYS: [&str; 5] = ["event", "hash", "seq", "writer", "written_at"];

/// Wynik weryfikacji łańcucha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainSummary {
    /// Liczba rekordów.
    pub records: u64,
    /// Hash ostatniego rekordu (głowa), jeśli są rekordy.
    pub head: Option<String>,
}

/// Naruszenia łańcucha (numer linii od 0 = `seq`).
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ChainError {
    /// Błąd odczytu pliku.
    #[error("odczyt łańcucha audytu: {0}")]
    Io(String),
    /// Plik nie kończy się znakiem nowej linii (urwany lub zmieniony ostatni bajt).
    #[error("łańcuch audytu urwany (brak końcowego znaku nowej linii)")]
    Truncated,
    /// Linia nie jest poprawnym rekordem.
    #[error("linia {line}: rekord uszkodzony ({reason})")]
    Malformed {
        /// Numer linii.
        line: u64,
        /// Powód.
        reason: String,
    },
    /// Linia nie jest w postaci kanonicznej (zmieniona forma zapisu).
    #[error("linia {line}: zapis niekanoniczny")]
    NotCanonical {
        /// Numer linii.
        line: u64,
    },
    /// Numer sekwencyjny nie zgadza się z pozycją (usunięcie/wstawienie).
    #[error("linia {line}: seq {found}, oczekiwano {line}")]
    SeqMismatch {
        /// Numer linii.
        line: u64,
        /// Znaleziony `seq`.
        found: u64,
    },
    /// Rekord nie pochodzi od writera pre-broker.
    #[error("linia {line}: nieoczekiwany writer")]
    WriterMismatch {
        /// Numer linii.
        line: u64,
    },
    /// `prev_hash` nie wskazuje poprzedniego rekordu.
    #[error("linia {line}: prev_hash nie pasuje do poprzedniego rekordu")]
    PrevHashMismatch {
        /// Numer linii.
        line: u64,
    },
    /// Hash rekordu nie zgadza się z treścią (modyfikacja).
    #[error("linia {line}: hash nie zgadza się z treścią")]
    HashMismatch {
        /// Numer linii.
        line: u64,
    },
    /// Głowa łańcucha różni się od oczekiwanej (np. ucięty ogon).
    #[error("głowa łańcucha różni się od oczekiwanej")]
    HeadMismatch,
}

fn malformed(line: u64, reason: impl Into<String>) -> ChainError {
    ChainError::Malformed {
        line,
        reason: reason.into(),
    }
}

fn verify_line(n: u64, raw: &[u8], prev: Option<&str>) -> Result<String, ChainError> {
    let text = std::str::from_utf8(raw).map_err(|_| malformed(n, "niepoprawny UTF-8"))?;
    let mut value: Value = serde_json::from_str(text).map_err(|e| malformed(n, e.to_string()))?;
    if canonical_json(&value) != text {
        return Err(ChainError::NotCanonical { line: n });
    }
    let obj = value
        .as_object_mut()
        .ok_or_else(|| malformed(n, "rekord nie jest obiektem"))?;
    if obj.len() != KEYS.len() || !KEYS.iter().all(|k| obj.contains_key(*k)) {
        return Err(malformed(n, "nieoczekiwany zestaw pól"));
    }
    let stored = match obj.remove("hash") {
        Some(Value::String(h)) => h,
        _ => return Err(malformed(n, "pole hash nie jest napisem")),
    };
    let seq = obj.get("seq").and_then(Value::as_u64);
    let seq = seq.ok_or_else(|| malformed(n, "pole seq"))?;
    if seq != n {
        return Err(ChainError::SeqMismatch {
            line: n,
            found: seq,
        });
    }
    if obj.get("writer").and_then(Value::as_str) != Some(PRE_BROKER_WRITER) {
        return Err(ChainError::WriterMismatch { line: n });
    }
    let written_at = obj.get("written_at").cloned().unwrap_or_default();
    serde_json::from_value::<chrono::DateTime<chrono::Utc>>(written_at)
        .map_err(|e| malformed(n, format!("written_at: {e}")))?;
    let event_value = obj.get("event").cloned().unwrap_or_default();
    let event: Event =
        serde_json::from_value(event_value).map_err(|e| malformed(n, format!("event: {e}")))?;
    if event.prev_hash.as_deref() != prev {
        return Err(ChainError::PrevHashMismatch { line: n });
    }
    if sha256_hex(canonical_json(&value).as_bytes()) != stored {
        return Err(ChainError::HashMismatch { line: n });
    }
    Ok(stored)
}

/// Weryfikuje łańcuch zapisany w bajtach (cały plik).
pub fn verify_bytes(bytes: &[u8]) -> Result<ChainSummary, ChainError> {
    let Some(body) = bytes.strip_suffix(b"\n") else {
        return if bytes.is_empty() {
            Ok(ChainSummary {
                records: 0,
                head: None,
            })
        } else {
            Err(ChainError::Truncated)
        };
    };
    let mut head: Option<String> = None;
    let mut records = 0u64;
    for (n, raw) in (0u64..).zip(body.split(|b| *b == b'\n')) {
        head = Some(verify_line(n, raw, head.as_deref())?);
        records = n + 1;
    }
    Ok(ChainSummary { records, head })
}

/// Weryfikuje plik łańcucha (brak pliku = pusty łańcuch).
pub fn verify_file(path: &Path) -> Result<ChainSummary, ChainError> {
    match fs::read(path) {
        Ok(bytes) => verify_bytes(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => verify_bytes(&[]),
        Err(e) => Err(ChainError::Io(e.to_string())),
    }
}

/// Weryfikuje plik i porównuje głowę z kotwicą przechowywaną poza plikiem.
pub fn verify_against(
    path: &Path,
    expected_head: Option<&str>,
) -> Result<ChainSummary, ChainError> {
    let summary = verify_file(path)?;
    if summary.head.as_deref() != expected_head {
        return Err(ChainError::HeadMismatch);
    }
    Ok(summary)
}

/// Błąd otwarcia writera Audytu.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum AuditOpenError {
    /// Istniejący łańcuch jest naruszony — nie dopisujemy do niego (baner + Diagnostyka).
    #[error("istniejący łańcuch audytu jest naruszony: {0}")]
    Broken(ChainError),
    /// Błąd we/wy.
    #[error("{0}")]
    Io(LogError),
}

struct ChainState {
    next_seq: u64,
    head: Option<String>,
    file: Option<File>,
    /// Po błędzie zapisu plik może mieć urwaną linię — dalsze dopisywanie tylko po ponownym `open`.
    poisoned: bool,
}

/// Writer Audytu pre-broker: redakcja przed zapisem, łańcuch SHA-256, zapis append-only.
pub struct PreBrokerAuditWriter {
    path: PathBuf,
    redactor: Arc<dyn Redactor>,
    clock: Arc<dyn Clock>,
    state: Mutex<ChainState>,
}

impl PreBrokerAuditWriter {
    /// Otwiera łańcuch (weryfikuje istniejący plik; naruszony → `AuditOpenError::Broken`).
    pub fn open(
        path: impl Into<PathBuf>,
        redactor: Arc<dyn Redactor>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, AuditOpenError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AuditOpenError::Io(io_err("katalog audytu", e)))?;
        }
        let summary = verify_file(&path).map_err(AuditOpenError::Broken)?;
        Ok(Self {
            path,
            redactor,
            clock,
            state: Mutex::new(ChainState {
                next_seq: summary.records,
                head: summary.head,
                file: None,
                poisoned: false,
            }),
        })
    }

    /// Ścieżka pliku łańcucha.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Weryfikuje plik i zgodność jego głowy z głową w pamięci (wykrywa też ucięcie ogona).
    pub fn verify_chain(&self) -> Result<ChainSummary, ChainError> {
        let head = self.lock().head.clone();
        verify_against(&self.path, head.as_deref())
    }

    fn lock(&self) -> MutexGuard<'_, ChainState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn append(&self, event: &Event) -> Result<AuditRecordRef, LogError> {
        let mut clean = event.clone();
        self.redactor.redact_value(&mut clean.payload);
        let mut state = self.lock();
        if state.poisoned {
            return Err(LogError::Io(
                "łańcuch audytu po błędzie zapisu — wymagane ponowne otwarcie i weryfikacja".into(),
            ));
        }
        clean.prev_hash = state.head.clone();
        let seq = state.next_seq;
        let event_value = serde_json::to_value(&clean).map_err(|e| io_err("serializacja", e))?;
        let mut record = json!({
            "event": event_value,
            "seq": seq,
            "writer": PRE_BROKER_WRITER,
            "written_at": self.clock.now(),
        });
        let hash = sha256_hex(canonical_json(&record).as_bytes());
        if let Some(obj) = record.as_object_mut() {
            obj.insert("hash".into(), Value::String(hash.clone()));
        }
        let mut line = canonical_json(&record);
        line.push('\n');
        if state.file.is_none() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .map_err(|e| io_err("otwarcie łańcucha audytu", e))?;
            state.file = Some(file);
        }
        let file = state
            .file
            .as_mut()
            .ok_or_else(|| LogError::Io("brak pliku audytu".into()))?;
        if let Err(e) = file.write_all(line.as_bytes()).and_then(|()| file.flush()) {
            state.poisoned = true;
            return Err(io_err("zapis audytu", e));
        }
        state.next_seq += 1;
        state.head = Some(hash.clone());
        Ok(AuditRecordRef { seq, hash })
    }
}

#[async_trait]
impl AuditWriter for PreBrokerAuditWriter {
    async fn append_audit(&self, event: &Event) -> Result<AuditRecordRef, LogError> {
        self.append(event)
    }

    async fn head_hash(&self) -> Result<Option<String>, LogError> {
        Ok(self.lock().head.clone())
    }
}
