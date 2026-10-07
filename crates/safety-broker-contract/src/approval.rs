//! Prośby o zatwierdzenie, wyzwania dla Broker-UI i decyzje właściciela (PLAN §8.2, §14.6).

use risk_classifier_contract::{AutonomyLevel, CommandOrigin, Reversibility, RiskLevel, RuleId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::autonomy::AutonomyTarget;
use crate::capability::Capability;
use crate::policy::KernelPolicy;
use crate::proof::Nonce;
use crate::token::{CapToken, Holder};

/// Identyfikator prośby o zatwierdzenie.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct ApprovalId(pub u64);

/// Krok planu do zatwierdzenia (karta planu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanStepSummary {
    /// Zdolność kroku.
    pub capability: Capability,
    /// Opis kroku (zwykły tekst).
    pub description: String,
    /// Ryzyko kroku.
    pub risk: RiskLevel,
}

/// Czego dotyczy prośba.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalSubject {
    /// Pojedyncza akcja.
    Action {
        /// Zdolność.
        capability: Capability,
        /// Narzędzie.
        tool: String,
    },
    /// „Plan do zatwierdzenia” — wiele akcji jednym zatwierdzeniem (§14.6).
    Plan {
        /// Tytuł planu.
        title: String,
        /// Kroki wymagające zgody.
        steps: Vec<PlanStepSummary>,
    },
    /// Zmiana poziomu autonomii.
    Autonomy {
        /// Cel.
        target: AutonomyTarget,
        /// Poziom bieżący.
        from: AutonomyLevel,
        /// Poziom żądany.
        to: AutonomyLevel,
        /// Do kiedy (ms), jeśli „na czas”.
        until_ms: Option<u64>,
    },
    /// Zmiana polityk Jądra.
    Policy {
        /// Nowa polityka.
        policy: Box<KernelPolicy>,
    },
}

/// Treść karty zatwierdzenia (zwykły tekst — Broker-UI nie renderuje HTML/markdown).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalRequest {
    /// Identyfikator.
    pub id: ApprovalId,
    /// Kto prosi.
    pub holder: Holder,
    /// Przedmiot.
    pub subject: ApprovalSubject,
    /// Klasa ryzyka.
    pub risk: RiskLevel,
    /// Odwracalność.
    pub reversible: Reversibility,
    /// Źródło polecenia (głos → potwierdzenie nie-głosem).
    pub origin: CommandOrigin,
    /// Czy sesja jest `tainted`.
    pub tainted: bool,
    /// Czy wymaga potwierdzenia nie-głosem (zawsze spełnione w Broker-UI).
    pub non_voice: bool,
    /// Czy można udzielić „zawsze zezwalaj w tym zakresie”.
    pub grantable: bool,
    /// Czy wymagane jest Windows Hello (polityka `hello_required_for`).
    pub hello_required: bool,
    /// Reguły, które zadziałały.
    pub rules: Vec<RuleId>,
    /// Wyjaśnienie (po polsku).
    pub explanation: String,
    /// Utworzono (ms).
    pub created_at_ms: u64,
    /// Wygasa (ms).
    pub expires_at_ms: u64,
}

/// Wyzwanie dla Broker-UI: karta + jednorazowy nonce. Wysyłane WYŁĄCZNIE kanałem Broker-UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalChallenge {
    /// Karta.
    pub request: ApprovalRequest,
    /// Nonce, który musi wrócić w dowodzie fizycznego wejścia.
    pub nonce: Nonce,
}

/// Decyzja właściciela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Zezwól raz.
    Allow,
    /// „Zawsze zezwalaj w tym zakresie” do terminu — nigdy nie eskaluje do L4: nie zmienia
    /// poziomu i nie pokrywa reguł „każdego poziomu” (głos, taint, trifecta, admin, Jądro).
    AllowInScope {
        /// Zakres (ta sama rodzina co prośba).
        scope: Capability,
        /// Do kiedy (ms); Broker przycina do limitu polityki.
        until_ms: u64,
    },
    /// Odmów.
    Deny,
}

/// Stan prośby widziany przez proszącego.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Czeka na decyzję w Broker-UI.
    Pending,
    /// Zatwierdzona akcja — token do odebrania (raz).
    Approved {
        /// Token (dla akcji; `None` dla planu, autonomii i polityki).
        token: Option<Box<CapToken>>,
    },
    /// Odrzucona.
    Denied,
    /// Wygasła albo unieważniona (kill-switch).
    Expired,
}
