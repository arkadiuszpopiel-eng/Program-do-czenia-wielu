//! Zdarzenia zadania mostu. Gramatyka strumienia (testy kontraktowe):
//! `Started (ColdStart | SessionStarted | Plan | Step | ToolRequest | ToolFinished |
//! PermissionRequest | PermissionResolved | Output | FileChanged | Usage | Warning)* (Done | Error)` —
//! dokładnie jedno zdarzenie końcowe, zawsze ostatnie.
//!
//! Każde zdarzenie jest opakowane w [`AgentEventEnvelope`] z `unverified_by_alfa = true`:
//! treść pochodzi z procesu CLI („opaque worker”, §8.5) i w Audycie jest oznaczona jako
//! „niezależnie niezweryfikowana”.

use std::path::PathBuf;

use providers_contract::Usage;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::approval::{ApprovalDecision, PermissionRequest, PermissionRequestId};
use crate::error::BackendError;
use crate::task::{BridgeKind, SessionRef, TaskId};

/// Format tekstu wyjściowego.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Zwykły tekst.
    Text,
    /// Markdown (renderowany w Rust przez `lib-markdown`, nigdy surowy HTML).
    Markdown,
}

/// Rodzaj zmiany pliku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    /// Utworzony.
    Added,
    /// Zmieniony.
    Modified,
    /// Usunięty.
    Deleted,
}

/// Pozycja planu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanItem {
    /// Treść kroku.
    pub text: String,
    /// Czy zakończony.
    pub done: bool,
}

/// Wynik zadania (zgłoszony przez CLI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskResult {
    /// Tekst końcowy (Markdown).
    pub text: String,
    /// Czy CLI zgłosiło błąd wykonania (np. limit tur).
    pub is_error: bool,
    /// Podtyp wyniku CLI (np. `success`, `error_max_turns`).
    pub subtype: Option<String>,
    /// Sesja do wznowienia.
    pub session: Option<SessionRef>,
    /// Liczba tur.
    pub num_turns: Option<u32>,
    /// Czas trwania wg CLI (ms).
    pub duration_ms: Option<u64>,
}

/// Zdarzenie zadania.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Proces CLI uruchomiony.
    Started {
        /// Most.
        bridge: BridgeKind,
        /// Wersja CLI (przypięta).
        cli_version: String,
        /// Izolowany katalog roboczy.
        workdir: PathBuf,
    },
    /// Zimny start: od uruchomienia procesu do pierwszego zdarzenia CLI (ms).
    ColdStart {
        /// Czas w ms.
        ms: u64,
    },
    /// CLI podało identyfikator sesji (do wznowienia).
    SessionStarted {
        /// Odnośnik.
        session: SessionRef,
    },
    /// Plan / lista kroków (Claude `TodoWrite`, Codex plan).
    Plan {
        /// Kroki.
        items: Vec<PlanItem>,
    },
    /// Krok rozumowania / opis bieżącej czynności.
    Step {
        /// Opis.
        text: String,
    },
    /// CLI wywołuje narzędzie (natywne w worktree albo MCP).
    ToolRequest {
        /// Identyfikator wywołania w CLI.
        call_id: String,
        /// Narzędzie.
        tool: String,
        /// Argumenty (niezaufane).
        input: Value,
    },
    /// Wynik narzędzia.
    ToolFinished {
        /// Identyfikator wywołania.
        call_id: String,
        /// Czy błąd.
        is_error: bool,
        /// Skrót wyniku (≤ 2 000 znaków).
        preview: String,
    },
    /// Prośba o uprawnienie przekazana do kanału zatwierdzeń.
    PermissionRequest {
        /// Prośba.
        request: PermissionRequest,
    },
    /// Decyzja w sprawie prośby.
    PermissionResolved {
        /// Prośba.
        id: PermissionRequestId,
        /// Decyzja.
        decision: ApprovalDecision,
        /// Czy decyzja wynika z limitu czasu (odmowa domyślna).
        timed_out: bool,
    },
    /// Tekst dla użytkownika.
    Output {
        /// Format.
        format: OutputFormat,
        /// Treść (przy `partial` — fragment).
        text: String,
        /// Fragment strumieniowany (pełna wiadomość przyjdzie osobno).
        partial: bool,
    },
    /// Zmiana pliku w katalogu roboczym.
    FileChanged {
        /// Ścieżka (tak, jak podało CLI).
        path: PathBuf,
        /// Rodzaj.
        change: FileChangeKind,
    },
    /// Zużycie (skumulowane) i koszt, jeśli CLI go podało.
    Usage {
        /// Tokeny.
        usage: Usage,
        /// Koszt w mikro-USD.
        cost_micro_usd: Option<u64>,
    },
    /// Nieznany typ wiadomości, linia spoza protokołu itp. — zalogowane, bez przerywania zadania.
    Warning {
        /// Opis (bez treści linii).
        message: String,
    },
    /// Zadanie zakończone wynikiem CLI (zdarzenie końcowe).
    Done {
        /// Wynik.
        result: TaskResult,
    },
    /// Zadanie zakończone błędem: crash, kod ≠ 0, anulowanie, budżet (zdarzenie końcowe).
    Error {
        /// Błąd.
        error: BackendError,
    },
}

impl AgentEvent {
    /// Czy zdarzenie kończy strumień.
    pub fn is_terminal(&self) -> bool {
        matches!(self, AgentEvent::Done { .. } | AgentEvent::Error { .. })
    }
}

/// Koperta zdarzenia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentEventEnvelope {
    /// Zadanie.
    pub task: TaskId,
    /// Numer kolejny w zadaniu (od 0, bez luk).
    pub seq: u64,
    /// Czas od przyjęcia zadania (ms, zegar monotoniczny backendu).
    pub at_ms: u64,
    /// Zawsze `true`: treść pochodzi z nieprzezroczystego procesu CLI (§8.5).
    pub unverified_by_alfa: bool,
    /// Zdarzenie.
    pub event: AgentEvent,
}

impl AgentEventEnvelope {
    /// Koperta z `unverified_by_alfa = true`.
    pub fn new(task: TaskId, seq: u64, at_ms: u64, event: AgentEvent) -> Self {
        Self {
            task,
            seq,
            at_ms,
            unverified_by_alfa: true,
            event,
        }
    }
}

/// Maksymalna długość skrótu wyniku narzędzia.
pub const MAX_PREVIEW_CHARS: usize = 2000;

/// Skraca tekst do `MAX_PREVIEW_CHARS` znaków (z wielokropkiem).
pub fn preview(text: &str) -> String {
    if text.chars().count() <= MAX_PREVIEW_CHARS {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(MAX_PREVIEW_CHARS).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_is_always_unverified_and_terminal_flags() {
        let e = AgentEventEnvelope::new(TaskId("t".into()), 0, 0, AgentEvent::ColdStart { ms: 5 });
        assert!(e.unverified_by_alfa);
        assert!(!e.event.is_terminal());
        assert!(
            AgentEvent::Error {
                error: BackendError::Cancelled
            }
            .is_terminal()
        );
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["event"]["type"], "cold_start");
        assert_eq!(json["unverified_by_alfa"], true);
    }

    #[test]
    fn preview_truncates_on_chars() {
        assert_eq!(preview("ąę"), "ąę");
        let long = "ż".repeat(MAX_PREVIEW_CHARS + 5);
        let p = preview(&long);
        assert_eq!(p.chars().count(), MAX_PREVIEW_CHARS + 1);
        assert!(p.ends_with('…'));
    }
}
