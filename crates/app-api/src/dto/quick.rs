//! DTO strony Ustawienia → Polecenia bez modelu (własne polecenia, katalog wbudowanych, tester)
//! — `types-quick.ts`.

use serde::{Deserialize, Serialize};

/// Tryb dopasowania frazy własnego polecenia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuickMatchMode {
    /// Cała wiadomość.
    Exact,
    /// Początek wiadomości; dalszy tekst to `{reszta}`.
    Starts,
    /// Fraza w wiadomości (pełne słowa).
    Contains,
    /// Wyrażenie regularne; nazwane grupy to zmienne.
    Regex,
}

/// Własne polecenie: frazy → stała odpowiedź.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CustomCommand {
    /// Identyfikator (pusty przy dodawaniu — nadaje rdzeń).
    pub id: String,
    pub name: String,
    pub phrases: Vec<String>,
    pub mode: QuickMatchMode,
    /// Odpowiedź (Markdown) z `{reszta}` i nazwanymi grupami wyrażenia.
    pub reply: String,
    pub enabled: bool,
    /// Wyższy wygrywa (−10…10).
    pub priority: i32,
}

/// Polecenie wbudowane (opis i przykłady).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BuiltinCommand {
    pub title: String,
    pub examples: Vec<String>,
}

/// Widok strony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QuickCommandsView {
    pub commands: Vec<CustomCommand>,
    pub builtins: Vec<BuiltinCommand>,
    /// Polecenia bez modelu włączone (`sessions.local_commands`).
    pub enabled: bool,
}

/// Kto odpowie na wiadomość.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuickSource {
    /// Własne polecenie.
    Custom,
    /// Polecenie wbudowane.
    Builtin,
    /// Początek quizu.
    Quiz,
    /// Model (żadne polecenie nie pasuje).
    Model,
    /// Polecenia wyłączone w ustawieniach.
    Disabled,
}

/// Wynik testera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QuickTest {
    pub source: QuickSource,
    /// Nazwa polecenia (własne) albo rodzaj (wbudowane).
    pub name: Option<String>,
    /// Odpowiedź, którą dostanie użytkownik.
    pub reply: Option<String>,
}
