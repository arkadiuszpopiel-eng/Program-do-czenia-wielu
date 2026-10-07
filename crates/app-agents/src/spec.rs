//! Specyfikacja przebiegu z obsady i Ustawień → Agentki: persona i role agentki (prompt
//! w rodzaju żeńskim z obsady), narzędzia, budżety (kroki, czas, koszt w PLN → mikro-USD
//! kursem NBP), limit czekania na zatwierdzenie, samoweryfikacja.

use agent_runtime_contract::{RunBudget, RunSpec};
use app_api::dto::{Money, RunBudgetView};
use core_bus_contract::{AgentId, SessionId};
use personas_contract::{Persona, Role};
use providers_contract::Message;
use risk_classifier_contract::CommandOrigin;

/// Klucze ustawień agentek (`data/settings-pages.json`, strona „Agentki").
pub mod keys {
    /// Maksymalna liczba kroków przebiegu.
    pub const MAX_STEPS: &str = "agents.max_steps";
    /// Maksymalny czas przebiegu (minuty).
    pub const MAX_MINUTES: &str = "agents.max_minutes";
    /// Maksymalny koszt przebiegu (PLN; 0 = bez limitu).
    pub const MAX_COST_PLN: &str = "agents.max_cost_pln";
    /// Limit czekania na zatwierdzenie w oknie Brokera (s).
    pub const APPROVAL_TIMEOUT_S: &str = "agents.approval_timeout_s";
    /// „Gotowe" dopiero po samoweryfikacji.
    pub const VERIFY: &str = "agents.verify_before_done";
}

/// Limit czekania na zatwierdzenie bez okna Brokera (tryb deweloperski): prośby i tak nie da
/// się zatwierdzić, więc agentka dostaje odmowę szybciej.
pub const NO_WINDOW_APPROVAL_CAP_S: u32 = 60;

/// Ustawienia agentek.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentSettings {
    /// Kroki.
    pub max_steps: u32,
    /// Minuty.
    pub max_minutes: u32,
    /// Koszt w groszach (0 = bez limitu).
    pub max_cost_grosze: u64,
    /// Limit czekania na zatwierdzenie (s).
    pub approval_timeout_s: u32,
    /// Samoweryfikacja.
    pub verify: bool,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_steps: 40,
            max_minutes: 15,
            max_cost_grosze: 0,
            approval_timeout_s: 300,
            verify: true,
        }
    }
}

impl AgentSettings {
    /// Budżet przebiegu; koszt PLN → mikro-USD kursem `usd_pln_e4` (USD→PLN × 10⁴).
    pub fn budget(&self, usd_pln_e4: u64) -> RunBudget {
        let max_cost_micro_usd = (self.max_cost_grosze > 0 && usd_pln_e4 > 0).then(|| {
            let micro_pln = u128::from(self.max_cost_grosze) * 10_000;
            let micro_usd = micro_pln * 10_000 / u128::from(usd_pln_e4);
            u64::try_from(micro_usd).unwrap_or(u64::MAX).max(1)
        });
        RunBudget {
            max_steps: self.max_steps.max(1),
            max_wall_ms: u64::from(self.max_minutes.max(1)) * 60_000,
            max_cost_micro_usd,
            ..RunBudget::default()
        }
    }

    /// Widok budżetu w UI.
    pub fn view(&self) -> RunBudgetView {
        RunBudgetView {
            max_steps: self.max_steps.max(1),
            max_minutes: self.max_minutes.max(1),
            max_cost: (self.max_cost_grosze > 0)
                .then(|| Money::pln(i64::try_from(self.max_cost_grosze).unwrap_or(i64::MAX))),
        }
    }

    /// Limit czekania na zatwierdzenie (ms) — bez okna Brokera przycięty.
    pub fn approval_timeout_ms(&self, broker_window: bool) -> u64 {
        let s = if broker_window {
            self.approval_timeout_s
        } else {
            self.approval_timeout_s.min(NO_WINDOW_APPROVAL_CAP_S)
        };
        u64::from(s.max(1)) * 1000
    }
}

/// Dane przebiegu z czatu.
#[derive(Debug, Clone)]
pub struct SpecInput {
    /// Sesja.
    pub session: SessionId,
    /// Agentka.
    pub persona: Persona,
    /// Role agentki w obsadzie sesji.
    pub roles: Vec<Role>,
    /// Polecenie właściciela.
    pub goal: String,
    /// Tekst / głos.
    pub origin: CommandOrigin,
    /// Model wybrany przez Router.
    pub model: String,
    /// Wszystkie narzędzia (runtime przecina je z grupami ról).
    pub tools: Vec<String>,
    /// Katalog roboczy sesji.
    pub workdir: String,
    /// Wcześniejsza rozmowa (gałąź append-only, bez bieżącej wiadomości).
    pub history: Vec<Message>,
}

/// `RunSpec` dla przebiegu.
pub fn run_spec(
    input: SpecInput,
    settings: &AgentSettings,
    usd_pln_e4: u64,
    broker_window: bool,
) -> RunSpec {
    RunSpec {
        session: input.session,
        agent: AgentId::new(input.persona.id.as_str()),
        persona: input.persona,
        roles: input.roles,
        goal: input.goal,
        origin: input.origin,
        model: input.model,
        tools: input.tools,
        budget: settings.budget(usd_pln_e4),
        workdir: Some(input.workdir),
        verify: settings.verify,
        approval_timeout_ms: settings.approval_timeout_ms(broker_window),
        history: input.history,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_from_settings() {
        let s = AgentSettings {
            max_cost_grosze: 400,
            ..AgentSettings::default()
        };
        let b = s.budget(40_000);
        assert_eq!(b.max_steps, 40);
        assert_eq!(b.max_wall_ms, 15 * 60_000);
        // 4 zł przy kursie 4,00 = 1 USD = 1 000 000 mikro-USD.
        assert_eq!(b.max_cost_micro_usd, Some(1_000_000));
        assert_eq!(
            AgentSettings::default().budget(40_000).max_cost_micro_usd,
            None
        );
        assert_eq!(s.view().max_cost, Some(Money::pln(400)));
        assert_eq!(s.approval_timeout_ms(true), 300_000);
        assert_eq!(s.approval_timeout_ms(false), 60_000);
        let zero = AgentSettings {
            max_steps: 0,
            max_minutes: 0,
            approval_timeout_s: 0,
            ..AgentSettings::default()
        };
        assert_eq!(zero.budget(1).max_steps, 1);
        assert_eq!(zero.approval_timeout_ms(true), 1000);
    }
}
