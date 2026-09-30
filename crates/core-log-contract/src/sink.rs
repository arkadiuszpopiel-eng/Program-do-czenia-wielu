//! Traity zapisu logów: `LogSink` (jądro) i `AuditWriter` (Broker).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use core_bus_contract::{Event, EventKind, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Strumienie zapisywane przez `core-log` (bez Audytu — ten pisze Broker, `AuditWriter`).
/// Porządek (`Ord`) = kolejność deklaracji; rozstrzyga remisy przy zapytaniu o wiele strumieni.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    /// Wywołania modeli: dostawca, model, tokeny, koszt, opóźnienie.
    ModelCalls,
    /// Narzędzia i GUI: krok + zrzut + migawka UIA (retencja domyślnie 7 dni).
    ToolsGui,
    /// Głos: opóźnienia etapów, fałszywe przerwania, WER.
    Voice,
    /// Diagnostyka: błędy, moduły, watchdog.
    Diagnostics,
}

impl LogStream {
    /// Wszystkie strumienie w kolejności deklaracji.
    pub const ALL: [LogStream; 4] = [
        LogStream::ModelCalls,
        LogStream::ToolsGui,
        LogStream::Voice,
        LogStream::Diagnostics,
    ];

    /// Domyślny strumień dla rodzaju zdarzenia; `None` dla Audytu (Broker) i UI.
    pub fn for_kind(kind: &EventKind) -> Option<Self> {
        match kind {
            EventKind::ModelCall => Some(Self::ModelCalls),
            EventKind::Tool | EventKind::Gui => Some(Self::ToolsGui),
            EventKind::Voice => Some(Self::Voice),
            EventKind::Diagnostics => Some(Self::Diagnostics),
            EventKind::Audit | EventKind::Ui | EventKind::Custom(_) => None,
        }
    }
}

/// Referencja do zapisanego rekordu (do `payload_ref` w zdarzeniach).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct RecordRef {
    /// Strumień.
    pub stream: LogStream,
    /// Numer sekwencyjny w strumieniu (monotoniczny).
    pub seq: u64,
}

/// Referencja do rekordu Audytu wraz z hashem łańcucha.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AuditRecordRef {
    /// Numer sekwencyjny.
    pub seq: u64,
    /// Hash tego rekordu (hex); staje się `prev_hash` następnego.
    pub hash: String,
}

/// Rekord zwracany przez `query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogRecord {
    /// Referencja.
    pub reference: RecordRef,
    /// Zdarzenie (po redakcji).
    pub event: Event,
    /// Wersja schematu zdarzenia w chwili zapisu.
    pub schema_version: u32,
}

/// Zapytanie o rekordy. Wynik: rosnąco po (`event.ts`, strumień, `seq`); `limit` na końcu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogQuery {
    /// Strumień; `None` = wszystkie strumienie `LogSink` (scalone).
    pub stream: Option<LogStream>,
    /// Tylko sesja.
    pub session: Option<SessionId>,
    /// Od numeru sekwencyjnego (włącznie; ma sens przy jednym strumieniu).
    pub from_seq: Option<u64>,
    /// Maksymalna liczba rekordów.
    pub limit: Option<usize>,
    /// Tylko rodzaj zdarzenia.
    #[serde(default)]
    pub kind: Option<EventKind>,
    /// Od czasu zdarzenia (`event.ts`, włącznie).
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
    /// Do czasu zdarzenia (`event.ts`, wyłącznie).
    #[serde(default)]
    pub until: Option<DateTime<Utc>>,
}

impl LogQuery {
    /// Czy rekord spełnia filtry zapytania (bez `limit`). Wspólne dla `-impl` i `-fake`.
    pub fn matches(&self, reference: &RecordRef, event: &Event) -> bool {
        self.stream.is_none_or(|s| s == reference.stream)
            && self.from_seq.is_none_or(|from| reference.seq >= from)
            && self
                .session
                .as_ref()
                .is_none_or(|s| event.session.as_ref() == Some(s))
            && self.kind.as_ref().is_none_or(|k| *k == event.kind)
            && self.since.is_none_or(|t| event.ts >= t)
            && self.until.is_none_or(|t| event.ts < t)
    }
}

/// Błędy logów.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum LogError {
    /// Przekroczono limit dysku; strumień zdegradowany.
    #[error("limit dysku osiągnięty dla strumienia {0:?}")]
    DiskLimit(LogStream),
    /// Błąd we/wy.
    #[error("błąd zapisu logu: {0}")]
    Io(String),
    /// Zdarzenie na deny-liście (aplikacja/okno/URL) — nie zapisano.
    #[error("zdarzenie odrzucone przez deny-listę: {0}")]
    Denylisted(String),
    /// Zapis Audytu nieautoryzowany (tylko Broker).
    #[error("zapis audytu nieautoryzowany")]
    AuditUnauthorized,
}

/// Append-only writer strumieni jądra. Brak metod modyfikujących i usuwających — celowo.
#[async_trait]
pub trait LogSink: Send + Sync {
    /// Dopisuje zdarzenie (po redakcji) na koniec strumienia.
    async fn append(&self, stream: LogStream, event: &Event) -> Result<RecordRef, LogError>;

    /// Odczyt rekordów wg zapytania.
    async fn query(&self, query: LogQuery) -> Result<Vec<LogRecord>, LogError>;
}

/// Writer strumienia Audyt — implementuje wyłącznie Broker (PLAN §8.1). Łańcuch hashy.
#[async_trait]
pub trait AuditWriter: Send + Sync {
    /// Dopisuje zdarzenie audytowe; zwraca hash rekordu do kotwiczenia łańcucha.
    async fn append_audit(&self, event: &Event) -> Result<AuditRecordRef, LogError>;

    /// Hash głowy łańcucha (ostatniego rekordu), jeśli łańcuch nie jest pusty.
    async fn head_hash(&self) -> Result<Option<String>, LogError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_matches_all_filters() {
        let t0 = chrono::DateTime::<Utc>::from_timestamp(1_000, 0).unwrap();
        let mut ev = Event::new(EventKind::Voice, core_bus_contract::Level::Info, 1.into())
            .with_session(SessionId::from("s1"));
        ev.ts = t0;
        let r = RecordRef {
            stream: LogStream::Voice,
            seq: 5,
        };
        assert!(LogQuery::default().matches(&r, &ev));
        let q = LogQuery {
            stream: Some(LogStream::Voice),
            session: Some(SessionId::from("s1")),
            from_seq: Some(5),
            kind: Some(EventKind::Voice),
            since: Some(t0),
            until: Some(t0 + chrono::Duration::seconds(1)),
            ..LogQuery::default()
        };
        assert!(q.matches(&r, &ev));
        let miss = [
            LogQuery {
                stream: Some(LogStream::Diagnostics),
                ..q.clone()
            },
            LogQuery {
                session: Some(SessionId::from("s2")),
                ..q.clone()
            },
            LogQuery {
                from_seq: Some(6),
                ..q.clone()
            },
            LogQuery {
                kind: Some(EventKind::Tool),
                ..q.clone()
            },
            LogQuery {
                since: Some(t0 + chrono::Duration::seconds(1)),
                ..q.clone()
            },
            LogQuery {
                until: Some(t0),
                ..q.clone()
            },
        ];
        for m in miss {
            assert!(!m.matches(&r, &ev), "{m:?}");
        }
        assert!(LogStream::ModelCalls < LogStream::Diagnostics);
    }

    #[test]
    fn stream_mapping() {
        assert_eq!(
            LogStream::for_kind(&EventKind::Gui),
            Some(LogStream::ToolsGui)
        );
        assert_eq!(LogStream::for_kind(&EventKind::Audit), None);
        assert_eq!(
            serde_json::to_string(&LogStream::ModelCalls).unwrap(),
            "\"model_calls\""
        );
    }
}
