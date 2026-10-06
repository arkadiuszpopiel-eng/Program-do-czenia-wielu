//! Mapowania kontraktu `agent-runtime`/`tools-*` na DTO UI: rodzaje i stany kroków, ikony,
//! intencje („uruchom w terminalu", trwałe usunięcie), tokeny cofania, tekst końcowy tury.

use agent_runtime_contract::{RunOutcome, StepKind, StepStatus};
use app_api::dto::{
    IntentKind, ReplayKind, ReplayStatus, RiskLevel, RunState, StopReason, ToolIcon, ToolIntent,
    ToolStatus as UiToolStatus, TurnError, TurnErrorCode, TurnStatus,
};
use app_api::ids;
use core_bus_contract::SessionId;
use tools_common_contract::{UndoRef, UndoService};
use tools_fs_contract::INTENT_CONFIRM_DELETE_PERMANENT;
use tools_shell_contract::INTENT_OPEN_IN_TERMINAL;

/// Rodzaj kroku Replay.
pub fn replay_kind(kind: StepKind) -> ReplayKind {
    match kind {
        StepKind::Plan => ReplayKind::Plan,
        StepKind::Think => ReplayKind::Think,
        StepKind::Tool => ReplayKind::Tool,
        StepKind::Verify => ReplayKind::Verify,
    }
}

/// Tytuł kroku modelu (narzędzia mają tytuł z manifestu).
pub fn model_step_title(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Plan => "Plan",
        StepKind::Think => "Decyzja",
        StepKind::Verify => "Weryfikacja",
        StepKind::Tool => "Narzędzie",
    }
}

/// Stan kroku Replay po zakończeniu.
pub fn replay_status(status: StepStatus) -> ReplayStatus {
    match status {
        StepStatus::Ok => ReplayStatus::Ok,
        StepStatus::Denied => ReplayStatus::Denied,
        StepStatus::NeedsConfirmation => ReplayStatus::NeedsConfirmation,
        StepStatus::Failed => ReplayStatus::Failed,
        StepStatus::Cancelled => ReplayStatus::Cancelled,
    }
}

/// Stan kroku narzędzia w wątku (zwinięta linia statusu).
pub fn tool_status(status: StepStatus) -> UiToolStatus {
    match status {
        StepStatus::Ok | StepStatus::NeedsConfirmation => UiToolStatus::Done,
        StepStatus::Denied | StepStatus::Failed | StepStatus::Cancelled => UiToolStatus::Error,
    }
}

/// Ikona narzędzia.
pub fn tool_icon(tool: &str) -> ToolIcon {
    match tool {
        "fs_search" => ToolIcon::Search,
        "fs_list" | "fs_read" | "fs_stat" | "clipboard_read" => ToolIcon::File,
        t if t.starts_with("shell_") => ToolIcon::Terminal,
        _ => ToolIcon::Edit,
    }
}

/// Klasa ryzyka Brokera → karta w UI (krytyczne pokazujemy jako wysokie).
pub fn risk(level: risk_classifier_contract::RiskLevel) -> RiskLevel {
    match level {
        risk_classifier_contract::RiskLevel::Low => RiskLevel::Low,
        risk_classifier_contract::RiskLevel::Medium => RiskLevel::Medium,
        risk_classifier_contract::RiskLevel::High
        | risk_classifier_contract::RiskLevel::Critical => RiskLevel::High,
    }
}

/// Token „Cofnij" kroku w DTO (dziennik, schowek albo zmienna użytkownika).
pub fn undo_token(session: &SessionId, undo: &UndoRef) -> String {
    match undo.service {
        UndoService::Journal => ids::undo_dto(session, undo.id),
        UndoService::Clipboard => ids::undo_clip_dto(session, undo.id),
        UndoService::System => ids::undo_env_dto(session, undo.id),
    }
}

fn text_field(details: &serde_json::Value, key: &str) -> Option<String> {
    details
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// Intencja narzędzia → karta UI (nieznane rodzaje są pomijane — UI nie wykonuje nic sam).
pub fn intent(intent: &tools_common_contract::ToolIntent) -> Option<ToolIntent> {
    let d = &intent.details;
    match intent.kind.as_str() {
        INTENT_OPEN_IN_TERMINAL => Some(ToolIntent {
            kind: IntentKind::OpenInTerminal,
            title: intent.title.clone(),
            command: text_field(d, "command"),
            cwd: text_field(d, "cwd"),
            shell: text_field(d, "shell"),
            paths: Vec::new(),
        }),
        INTENT_CONFIRM_DELETE_PERMANENT => Some(ToolIntent {
            kind: IntentKind::ConfirmDeletePermanent,
            title: intent.title.clone(),
            command: None,
            cwd: None,
            shell: None,
            paths: d
                .get("paths")
                .and_then(serde_json::Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        }),
        _ => None,
    }
}

/// Stan przebiegu z wyniku.
pub fn run_state(outcome: &RunOutcome) -> RunState {
    match outcome {
        RunOutcome::Completed { .. } => RunState::Completed,
        RunOutcome::BudgetExceeded { .. } => RunState::BudgetExceeded,
        RunOutcome::Cancelled => RunState::Cancelled,
        RunOutcome::LoopDetected { .. } => RunState::LoopDetected,
        RunOutcome::Refused => RunState::Refused,
        RunOutcome::Failed { .. } => RunState::Failed,
    }
}

/// Treść tury agentki po przebiegu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalText {
    /// Tekst odpowiedzi (pusty = tura-komunikat z błędem).
    pub text: String,
    /// Stan tury.
    pub status: TurnStatus,
    /// Powód zakończenia (bez błędu).
    pub stop: Option<StopReason>,
    /// Błąd.
    pub error: Option<TurnError>,
}

fn stop(text: String, reason: StopReason) -> FinalText {
    FinalText {
        text,
        status: if reason == StopReason::Cancelled {
            TurnStatus::Cancelled
        } else {
            TurnStatus::Complete
        },
        stop: Some(reason),
        error: None,
    }
}

/// Wynik przebiegu → tekst i stan tury (rodzaj żeński; przyczyny zatrzymania wprost).
pub fn final_text(outcome: &RunOutcome) -> FinalText {
    match outcome {
        RunOutcome::Completed { summary, verified } => {
            let note = match verified {
                Some(false) => "\n\n_Weryfikacja wykryła problem — sprawdź wynik._",
                _ => "",
            };
            stop(format!("{}{note}", summary.trim()), StopReason::End)
        }
        RunOutcome::BudgetExceeded { budget } => stop(
            format!(
                "Zatrzymałam się: przekroczony budżet {} (Ustawienia → Agentki). Kroki są w Replay na Osi czasu.",
                budget.label_pl()
            ),
            StopReason::End,
        ),
        RunOutcome::LoopDetected { tool } => stop(
            format!(
                "Zatrzymałam się: to samo wywołanie `{tool}` powtarzało się (wykryta pętla). Doprecyzuj, proszę, zadanie."
            ),
            StopReason::End,
        ),
        RunOutcome::Cancelled => stop(String::new(), StopReason::Cancelled),
        RunOutcome::Refused => stop(String::new(), StopReason::Refusal),
        RunOutcome::Failed { error } => FinalText {
            text: String::new(),
            status: TurnStatus::Error,
            stop: None,
            error: Some(TurnError {
                code: TurnErrorCode::Provider,
                message: format!("Zadanie agentki przerwane: {error}"),
                retry_at: None,
                provider: None,
            }),
        },
    }
}

/// Skrót jednej linii (bez nowych linii, z wielokropkiem).
pub fn short(s: &str, max: usize) -> String {
    let line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= max {
        return line;
    }
    let cut: String = line.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_runtime_contract::BudgetKind;

    #[test]
    fn mappings() {
        assert_eq!(tool_icon("fs_search"), ToolIcon::Search);
        assert_eq!(tool_icon("shell_run"), ToolIcon::Terminal);
        assert_eq!(tool_icon("fs_write"), ToolIcon::Edit);
        assert_eq!(tool_status(StepStatus::Denied), UiToolStatus::Error);
        assert_eq!(replay_status(StepStatus::Ok), ReplayStatus::Ok);
        assert_eq!(replay_kind(StepKind::Verify), ReplayKind::Verify);
        assert_eq!(short("a\n b   c", 10), "a b c");
        assert_eq!(short("abcdefghij", 5), "abcd…");
        let s = SessionId::new("s1");
        let clip = UndoRef {
            service: UndoService::Clipboard,
            id: 3,
            text: String::new(),
        };
        assert_eq!(undo_token(&s, &clip), "s1:c3");
        let env = UndoRef {
            service: UndoService::System,
            id: 4,
            text: String::new(),
        };
        assert_eq!(undo_token(&s, &env), "s1:v4");
    }

    #[test]
    fn intents_and_final_texts() {
        let t = tools_common_contract::ToolIntent {
            kind: INTENT_OPEN_IN_TERMINAL.into(),
            title: "Uruchom w terminalu".into(),
            details: serde_json::json!({"shell": "pwsh", "command": "npm run dev", "cwd": "C:\\w"}),
        };
        let i = intent(&t).unwrap();
        assert_eq!(i.kind, IntentKind::OpenInTerminal);
        assert_eq!(i.command.as_deref(), Some("npm run dev"));
        let unknown = tools_common_contract::ToolIntent {
            kind: "x".into(),
            title: String::new(),
            details: serde_json::Value::Null,
        };
        assert!(intent(&unknown).is_none());
        let done = final_text(&RunOutcome::Completed {
            summary: "Gotowe.".into(),
            verified: Some(true),
        });
        assert_eq!(done.text, "Gotowe.");
        let budget = final_text(&RunOutcome::BudgetExceeded {
            budget: BudgetKind::Steps,
        });
        assert!(budget.text.contains("kroków"));
        assert_eq!(
            final_text(&RunOutcome::Cancelled).status,
            TurnStatus::Cancelled
        );
        assert!(
            final_text(&RunOutcome::Failed { error: "x".into() })
                .error
                .is_some()
        );
        assert_eq!(run_state(&RunOutcome::Refused), RunState::Refused);
    }
}
