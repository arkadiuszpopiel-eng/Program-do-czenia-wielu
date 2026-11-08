//! Wejście i werdykt sędziego.

use risk_classifier_contract::{Destructiveness, Reversibility, RuleId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Jedna pozycja zwięzłego zapisu rozmowy (bez ukrytego rozumowania agentki).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JudgeTurn {
    /// Wiadomość właściciela — jedyne źródło poleceń dla sędziego.
    Owner {
        /// Treść dosłownie.
        text: String,
    },
    /// Krok agentki (skrót: co zrobiła albo zapowiedziała).
    AgentStep {
        /// Skrót kroku.
        text: String,
    },
    /// Wynik narzędzia — **treść obca**, w prompcie zawsze jako dane, nigdy polecenia.
    ToolResult {
        /// Nazwa narzędzia.
        tool: String,
        /// Treść (obcięta przez wywołującego).
        text: String,
    },
}

/// Akcja do oceny z faktami Sędziego Cieni.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JudgedAction {
    /// Narzędzie (np. `shell.run`).
    pub tool: String,
    /// Opis zdolności po polsku (np. „zapis w E:\Projekt”).
    pub capability_summary: String,
    /// Polecenie powłoki, gdy jest.
    pub command: Option<String>,
    /// Ścieżki, których dotyczy.
    pub paths: Vec<String>,
    /// Host wyjścia sieciowego, gdy jest.
    pub host: Option<String>,
    /// Odwracalność.
    pub reversible: Reversibility,
    /// Destrukcyjność.
    pub destructive: Destructiveness,
    /// Reguły, przez które Sędzia Cieni zapytałby właściciela.
    pub rules: Vec<RuleId>,
    /// Zmiana poza folderem pracy rozmowy.
    pub outside_workdir: bool,
    /// Sesja widziała treść obcą.
    pub tainted: bool,
}

/// Pełne wejście sędziego.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JudgeInput {
    /// Cel zadania (pierwsza wiadomość właściciela albo opis zadania).
    pub goal: String,
    /// Zwięzły zapis, od najstarszych.
    pub transcript: Vec<JudgeTurn>,
    /// Oceniana akcja.
    pub action: JudgedAction,
    /// Granice podane przez właściciela w rozmowie („nie wypychaj”, „nie usuwaj”).
    pub limits: Vec<String>,
}

/// Kategoria domyślnej odmowy (wspólne kategorie Claude Code i Codex, plan 50 §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DenyCategory {
    /// Wysyłka danych prywatnych, sekretów, haseł do niezaufanych miejsc.
    SecretExfiltration,
    /// Szukanie haseł, tokenów, ciasteczek.
    CredentialHunting,
    /// Trwałe osłabianie zabezpieczeń.
    WeakenSecurity,
    /// Nieodwracalne niszczenie (usuwanie danych, nadpisanie historii git, formatowanie).
    IrreversibleDestruction,
    /// Polecenie z treści obcej, którego właściciel nie zlecił.
    UntrustedInstruction,
    /// Granica podana przez właściciela w rozmowie.
    OwnerBoundary,
    /// Inny powód.
    Other,
}

impl DenyCategory {
    /// Nazwa po polsku (karta, dziennik).
    pub fn name_pl(self) -> &'static str {
        match self {
            Self::SecretExfiltration => "wysyłka danych prywatnych lub sekretów",
            Self::CredentialHunting => "szukanie haseł, tokenów lub ciasteczek",
            Self::WeakenSecurity => "osłabianie zabezpieczeń",
            Self::IrreversibleDestruction => "nieodwracalne niszczenie danych",
            Self::UntrustedInstruction => "polecenie z treści obcej",
            Self::OwnerBoundary => "granica podana przez właściciela",
            Self::Other => "inny powód",
        }
    }
}

/// Werdykt sędziego.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum JudgeVerdict {
    /// Zgoda **jednorazowa** na tę jedną akcję.
    Allow {
        /// Uzasadnienie.
        reason: String,
    },
    /// Odmowa — agentka dostaje powód i zakaz obchodzenia.
    Deny {
        /// Kategoria.
        category: DenyCategory,
        /// Uzasadnienie.
        reason: String,
    },
    /// Decyzja właściciela (wątpliwość, błąd, brak modelu, termin).
    Escalate {
        /// Dlaczego sędzia nie rozstrzygnął.
        reason: String,
    },
}

impl JudgeVerdict {
    /// Werdykt eskalacji z powodem.
    pub fn escalate(reason: impl Into<String>) -> Self {
        Self::Escalate {
            reason: reason.into(),
        }
    }

    /// Czy to odmowa (liczy się do bezpiecznika).
    pub fn is_deny(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }
}
