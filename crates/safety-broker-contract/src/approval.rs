//! Prośby o zatwierdzenie, wyzwania dla Broker-UI i decyzje właściciela (PLAN §8.2, §14.6).

use core_bus_contract::{AgentId, SessionId};
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
        /// Co dokładnie narzędzie zrobi (z faktów narzędzia; zwykły tekst).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
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
    /// Tryb pracy, przy którym padło pytanie (okno mówi, czemu pyta; na L4 okna nie ma).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autonomy: Option<AutonomyLevel>,
    /// Okno może zaproponować „Zezwól i przełącz na Automatycznie”: pojedyncza akcja, tryb
    /// niższy niż L3, podmiot bez własnego poziomu (zmiana dotyczy trybu globalnego).
    #[serde(default)]
    pub auto_switch: bool,
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

/// Termin zgody „do odwołania” (właściciel 31.10: czas zgody wybiera użytkownik — 1 h, 3 h,
/// doba albo do odwołania w panelu uprawnień).
pub const UNTIL_REVOKED: u64 = u64::MAX;

/// Zgoda „zawsze w tym zakresie” widziana w panelu uprawnień (lista i odwołanie przez jądro).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GrantInfo {
    /// Numer zgody (do odwołania).
    pub id: u64,
    /// Sesja.
    pub session: SessionId,
    /// Agentka (brak — cała sesja).
    pub agent: Option<AgentId>,
    /// Zakres.
    pub scope: Capability,
    /// Kiedy udzielona (ms).
    pub granted_at_ms: u64,
    /// Do kiedy (ms); [`UNTIL_REVOKED`] — do odwołania.
    pub until_ms: u64,
}

/// Decyzja właściciela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Zezwól raz.
    Allow,
    /// „Zawsze zezwalaj w tym zakresie” na czas wybrany przez właściciela (1 h, 3 h, doba)
    /// albo do odwołania — nigdy nie eskaluje do L4: nie zmienia poziomu i nie pokrywa reguł
    /// „każdego poziomu” (głos, taint, trifecta, admin, Jądro).
    AllowInScope {
        /// Zakres (ta sama rodzina co prośba).
        scope: Capability,
        /// Do kiedy (ms); Broker przycina do limitu polityki. [`UNTIL_REVOKED`] — bez terminu,
        /// aż właściciel odwoła zgodę w Ustawieniach → Uprawnienia (albo STOP).
        until_ms: u64,
    },
    /// Odmów.
    Deny,
    /// Zezwól raz i przełącz tryb pracy na „Automatycznie” (L3) — opcja okna przy niższym trybie
    /// (plan 50 §4.2, jak w Claude Code). Tylko gdy prośba ma [`ApprovalRequest::auto_switch`].
    AllowAndAutomatic,
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
