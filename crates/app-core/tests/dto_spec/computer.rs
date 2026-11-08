//! Specyfikacje komend F8 dla `dto_roundtrip.rs`: panel „Ekran" (computer use), wbudowany terminal
//! (strumień w `Channel` — argument `channel` to identyfikator kanału), umiejętności, Kreator agentek
//! i „Zdrowie systemu".

use app_core::dto::*;

use crate::{Check, parse_only, roundtrip};

use super::Spec;

fn screen_and_terminal(command: &str) -> Option<Spec> {
    let unit: Check = roundtrip::<()>;
    let gui: Check = roundtrip::<GuiStatus>;
    let id: Check = roundtrip::<u64>;
    let size: Check = roundtrip::<u16>;
    Some(match command {
        "gui_status" | "gui_stop" | "gui_release" => (vec![], gui),
        "gui_screenshot" => (vec![], roundtrip::<Option<GuiScreenshot>>),
        "gui_desktop_grant" => (
            vec![
                ("sessionId", roundtrip::<String>),
                ("agent", roundtrip::<String>),
            ],
            roundtrip::<BrokerIntentResult>,
        ),
        "terminal_open" => (
            vec![
                ("profile", roundtrip::<TerminalProfileId>),
                ("cols", size),
                ("rows", size),
                ("cwd", roundtrip::<Option<String>>),
                ("channel", parse_only::<String>),
            ],
            roundtrip::<TerminalSession>,
        ),
        "terminal_input" => (
            vec![("terminal", id), ("dataB64", roundtrip::<String>)],
            unit,
        ),
        "terminal_resize" => (vec![("terminal", id), ("cols", size), ("rows", size)], unit),
        "terminal_close" => (vec![("terminal", id)], unit),
        "terminal_list" => (vec![], roundtrip::<Vec<TerminalSession>>),
        _ => return None,
    })
}

fn skills(command: &str) -> Option<Spec> {
    let s: Check = roundtrip::<String>;
    let skill: Check = roundtrip::<SkillInfo>;
    let draft: Check = roundtrip::<AgentDraft>;
    Some(match command {
        "skills_list" => (vec![], roundtrip::<Vec<SkillInfo>>),
        "skills_review" => (
            vec![("skillId", s), ("version", s)],
            roundtrip::<SkillReview>,
        ),
        "skills_propose" => (vec![("skill", roundtrip::<serde_json::Value>)], skill),
        "skills_approve" | "skills_release" => {
            (vec![("skillId", s), ("version", s), ("hash", s)], skill)
        }
        "skills_reject" => (vec![("skillId", s), ("version", s)], skill),
        "skills_disable" => (vec![("skillId", s)], skill),
        "skills_run" => (
            vec![
                ("skillId", s),
                ("sessionId", s),
                ("agent", roundtrip::<Option<String>>),
                ("params", roundtrip::<serde_json::Value>),
            ],
            roundtrip::<TaskInfo>,
        ),
        "skills_export" => (vec![], roundtrip::<ExportResult>),
        "skills_import" => (vec![], roundtrip::<SkillImportResult>),
        "builder_policy" => (vec![], roundtrip::<BuilderPolicyView>),
        "builder_propose" => (vec![("description", s)], roundtrip::<BuilderProposal>),
        "builder_preview" => (vec![("draft", draft)], roundtrip::<BuilderPreview>),
        "builder_dry_run" => (vec![("draft", draft)], roundtrip::<BuilderDryRun>),
        "builder_save" => (
            vec![("draft", draft), ("hash", s)],
            roundtrip::<BuilderSaved>,
        ),
        "builder_voice_preview" => (vec![("draft", draft)], roundtrip::<()>),
        "builder_library" => (vec![], roundtrip::<Vec<BuilderAgentInfo>>),
        _ => return None,
    })
}

fn health(command: &str) -> Option<Spec> {
    let view: Check = roundtrip::<HealthView>;
    let improver: Check = roundtrip::<ImproverView>;
    let id: Check = roundtrip::<u64>;
    Some(match command {
        "health_report" | "health_scan" => (vec![], view),
        "health_approve" | "health_reject" | "health_undo" => (vec![("repairId", id)], view),
        "improver_list" | "improver_cycle" => (vec![], improver),
        "improver_approve" => (
            vec![("proposalId", id), ("digest", roundtrip::<String>)],
            improver,
        ),
        "improver_reject" | "improver_rollback" => (vec![("proposalId", id)], improver),
        "evals_list" => (vec![], roundtrip::<EvalsView>),
        "evals_verify" => (
            vec![("suiteId", roundtrip::<String>)],
            roundtrip::<EvalSuiteView>,
        ),
        _ => return None,
    })
}

/// Specyfikacja komendy F8 (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    screen_and_terminal(command)
        .or_else(|| skills(command))
        .or_else(|| health(command))
}
