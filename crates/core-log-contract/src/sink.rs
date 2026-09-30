//! Traity zapisu logów: `LogSink` (jądro) i `AuditWriter` (Broker).

use async_trait::async_trait;
use core_bus_contract::{Event, EventKind, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Strumienie zapisywane przez `core-log` (bez Audytu — ten pisze Broker, `AuditWriter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
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

/// Zapytanie o rekordy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogQuery {
    /// Strumień (wymagany).
    pub stream: Option<LogStream>,
    /// Tylko sesja.
    pub session: Option<SessionId>,
    /// Od numeru sekwencyjnego (włącznie).
    pub from_seq: Option<u64>,
    /// Maksymalna liczba rekordów.
    pub limit: Option<usize>,
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
