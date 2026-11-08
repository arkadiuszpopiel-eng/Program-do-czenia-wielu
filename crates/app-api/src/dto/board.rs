//! DTO Tablicy — przypiętych odpowiedzi (`types-board.ts`).

use serde::{Deserialize, Serialize};

/// Przypięta odpowiedź (kopia treści z nazwą rozmowy, agentką i czasem przypięcia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BoardItem {
    pub id: String,
    pub session_id: String,
    pub session_title: String,
    pub turn_id: String,
    pub agent: String,
    /// Treść (Markdown) — do kopiowania.
    pub text: String,
    /// Treść jako HTML zsanitowany w Rust — tylko do `SanitizedHtml`.
    pub html_sanitized: String,
    pub pinned_at: String,
}
