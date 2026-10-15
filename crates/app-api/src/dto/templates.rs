//! DTO szablonów promptów (Ustawienia → Szablony promptów, okno „Wstaw szablon”) —
//! `types-templates.ts`.

use serde::{Deserialize, Serialize};

/// Szablon promptu: tekst ze zmiennymi `{{nazwa}}` do uzupełnienia przed wstawieniem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PromptTemplate {
    /// Identyfikator (pusty przy dodawaniu — nadaje rdzeń).
    pub id: String,
    pub name: String,
    /// Treść ze zmiennymi `{{nazwa}}`.
    pub text: String,
}
