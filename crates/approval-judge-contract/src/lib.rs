//! Kontrakt modelu-sędziego trybu „Automatycznie” (L3) — plan 50 §4.3, zadanie Z.4,
//! `docs/modules/approval-judge/SPEC.md`.
//!
//! Sędzia **zastępuje osobę zatwierdzającą, nie dodaje uprawnień**: ocenia tylko prośby, o które Sędzia Cieni
//! zapytałby właściciela na L3 (`eligible`), daje co najwyżej zgodę jednorazową, a każdy błąd kończy się kartą
//! właściciela (`JudgeVerdict::Escalate`) — nigdy cichą zgodą.
//!
//! Crate zawiera wyłącznie czyste funkcje i typy, żeby Broker (osobny proces, bez dostawców modeli) mógł użyć
//! `eligible` i `JudgeFuse`, a rdzeń — `render_prompt` i `parse_verdict`.

mod eligible;
mod fuse;
mod prompt;
mod types;

pub use eligible::{JUDGE_RULES, eligible};
pub use fuse::{FuseEvent, FuseLimits, JudgeFuse};
pub use prompt::{JudgePrompt, MAX_REASON_CHARS, parse_verdict, render_prompt};
pub use types::{DenyCategory, JudgeInput, JudgeTurn, JudgeVerdict, JudgedAction};

/// Model-sędzia. Implementacja nie może zwrócić `Allow` przy błędzie — błąd to `Escalate`.
#[async_trait::async_trait]
pub trait ApprovalJudge: Send + Sync {
    /// Ocena jednej prośby. Termin pilnuje wywołujący (kierowca sędziego) i Broker.
    async fn review(&self, input: &JudgeInput) -> JudgeVerdict;

    /// Nazwa modelu do dziennika (np. `local:bielik-4.5b`).
    fn model_name(&self) -> String;
}
