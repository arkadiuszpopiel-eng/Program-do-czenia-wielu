//! Specyfikacja przebiegu: kto (sesja, persona, role), cel, źródło polecenia, model,
//! narzędzia, budżety, katalog roboczy, weryfikacja.

use core_bus_contract::{AgentId, SessionId};
use personas_contract::{Persona, Role};
use providers_contract::Message;
use risk_classifier_contract::CommandOrigin;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Budżety przebiegu (`[agent] default_budget`); przekroczenie = czyste zatrzymanie z raportem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunBudget {
    /// Maksymalna liczba kroków atomowych (tura modelu albo wywołanie narzędzia).
    pub max_steps: u32,
    /// Maksymalna suma tokenów (wejście + wyjście).
    pub max_tokens: u64,
    /// Maksymalny czas przebiegu (ms, łącznie po wznowieniach).
    pub max_wall_ms: u64,
    /// Maksymalny koszt (mikro-USD); `None` = bez limitu (np. model lokalny).
    pub max_cost_micro_usd: Option<u64>,
    /// Maksymalna liczba wywołań narzędzi w jednej turze modelu (v0: sekwencyjnie).
    pub max_tool_calls_per_turn: u32,
}

impl Default for RunBudget {
    fn default() -> Self {
        Self {
            max_steps: 40,
            max_tokens: 200_000,
            max_wall_ms: 15 * 60 * 1000,
            max_cost_micro_usd: None,
            max_tool_calls_per_turn: 8,
        }
    }
}

/// Który budżet przekroczono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetKind {
    /// Kroki.
    Steps,
    /// Tokeny.
    Tokens,
    /// Czas.
    Wall,
    /// Koszt.
    Cost,
}

impl BudgetKind {
    /// Nazwa po polsku.
    pub fn label_pl(self) -> &'static str {
        match self {
            Self::Steps => "kroków",
            Self::Tokens => "tokenów",
            Self::Wall => "czasu",
            Self::Cost => "kosztu",
        }
    }
}

/// Specyfikacja przebiegu (v0: jedna agentka, narzędzia sekwencyjnie, bez DAG).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunSpec {
    /// Sesja.
    pub session: SessionId,
    /// Agentka (identyfikator persony).
    pub agent: AgentId,
    /// Persona (tożsamość, prompt w rodzaju żeńskim).
    pub persona: Persona,
    /// Role w obsadzie (narzędzia i uprawnienia idą za rolą).
    pub roles: Vec<Role>,
    /// Cel (polecenie właściciela).
    pub goal: String,
    /// Źródło polecenia (tekst/głos) — przechodzi do faktów Brokera.
    pub origin: CommandOrigin,
    /// Model u dostawcy.
    pub model: String,
    /// Narzędzia dozwolone w przebiegu (nazwy; przecięcie z grupami ról).
    pub tools: Vec<String>,
    /// Budżety.
    #[serde(default)]
    pub budget: RunBudget,
    /// Katalog roboczy.
    #[serde(default)]
    pub workdir: Option<String>,
    /// „Gotowe” dopiero po samoweryfikacji (`verify_before_done = true`).
    #[serde(default = "yes")]
    pub verify: bool,
    /// Limit czekania na zatwierdzenie w Broker-UI (ms).
    #[serde(default = "approval_timeout")]
    pub approval_timeout_ms: u64,
    /// Wcześniejsza rozmowa (projekcja gałęzi, append-only) — kontekst zadania.
    #[serde(default)]
    pub history: Vec<Message>,
}

fn yes() -> bool {
    true
}

fn approval_timeout() -> u64 {
    5 * 60 * 1000
}

impl RunSpec {
    /// Walidacja niezależna od implementacji.
    pub fn validate(&self) -> Result<(), String> {
        if self.goal.trim().is_empty() {
            return Err("pusty cel przebiegu".into());
        }
        if self.model.trim().is_empty() {
            return Err("brak modelu".into());
        }
        if self.session.as_str().is_empty() || self.agent.as_str().is_empty() {
            return Err("brak sesji albo agentki".into());
        }
        let b = &self.budget;
        if b.max_steps == 0
            || b.max_tokens == 0
            || b.max_wall_ms == 0
            || b.max_tool_calls_per_turn == 0
        {
            return Err("budżety muszą być dodatnie".into());
        }
        if self.approval_timeout_ms == 0 {
            return Err("limit czekania na zatwierdzenie musi być dodatni".into());
        }
        Ok(())
    }
}
