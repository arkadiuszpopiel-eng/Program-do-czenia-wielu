//! Język reguł Marszałka (PLAN §9.4): `id`, `when` (warunek), `then` (efekty). Typy dopuszczają
//! **wyłącznie** efekty zawężające — nie da się zapisać „pozwól”, „podnieś”, „wyłącz audyt”;
//! nieznane pola i efekty są odrzucane przy parsowaniu (szkice z LLM są niezaufane).

use std::fmt;

use personas_contract::{PersonaId, RoleId};
use safety_broker_contract::{AutonomyLevel, Capability};
use scheduler_contract::{OnTimeout, Priority, TaskClass};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identyfikator reguły: 1–64 znaki `[a-z0-9-]`.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RuleId(pub String);

impl RuleId {
    /// Poprawna postać.
    pub fn is_valid(&self) -> bool {
        (1..=64).contains(&self.0.len())
            && self
                .0
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RuleId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Rodzaj zasobu wyłącznego w warunku/efekcie.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    /// Głośnik / mówienie.
    Speaker,
    /// Mikrofon.
    Mic,
    /// Ekran + mysz/klawiatura.
    ScreenInput,
    /// Pliki.
    File,
}

/// Zdarzenie w warunku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Użytkownik mówi (voice-first).
    UserSpeaks,
}

/// Pewność rozpoznania w warunku `user_speaks`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Dowolna.
    Any,
    /// Co najmniej potwierdzona.
    Confirmed,
}

/// Pochodzenie zadania w warunku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    /// Użytkownik.
    User,
    /// Agentka.
    Agent,
    /// Wyzwalacz.
    Trigger,
    /// Harmonogram.
    Schedule,
}

/// Przedział minut doby (czas lokalny; `start > end` = przez północ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeRange {
    /// Początek (0–1439).
    pub start_min: u16,
    /// Koniec (0–1439, wyłącznie).
    pub end_min: u16,
}

/// Warunek: koniunkcja podanych pól (puste = zawsze).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct When {
    /// Zadanie używa zasobu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceKind>,
    /// Zdarzenie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<EventKind>,
    /// Pewność (dla `user_speaks`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    /// Agentka.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<PersonaId>,
    /// Rola.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<RoleId>,
    /// Klasa zadania.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_class: Option<TaskClass>,
    /// Pochodzenie zadania.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<OriginKind>,
    /// Zadanie z treścią niezaufaną.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tainted: Option<bool>,
    /// Pora dnia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<TimeRange>,
}

/// Kogo wstrzymywać w punkcie atomowym (PLAN §9.4: tylko agentki GUI/audio).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PauseScope {
    /// Zadania trzymające ekran i wejście.
    Gui,
    /// Zadania trzymające głośnik albo mikrofon.
    Audio,
}

/// Kiedy wznowić wstrzymane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResumeAfter {
    /// Po końcu tury użytkownika.
    TurnEnd,
}

/// Efekt reguły — wyłącznie zawężający.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    /// Wyłączność zasobu z kolejką priorytetową i limitem czekania (≤ 600 s).
    Exclusive {
        /// Zasób.
        resource: ResourceKind,
        /// Najdłuższe czekanie (ms).
        max_wait_ms: u64,
        /// Po czasie: zapytaj użytkownika albo błąd.
        on_timeout: OnTimeout,
    },
    /// Wywłaszcz zadania tych priorytetów (np. narrację, gdy mówi użytkownik).
    Preempt {
        /// Priorytety (najwyżej `interactive` — mowy użytkownika nie da się wywłaszczyć).
        classes: Vec<Priority>,
    },
    /// Wstrzymaj w punkcie atomowym.
    PauseAtAtomic {
        /// Zakres (GUI/audio).
        scope: PauseScope,
        /// Wznowienie.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume_after: Option<ResumeAfter>,
    },
    /// Odmawiaj zdolności (zakres).
    DenyCapability {
        /// Zdolność.
        capability: Capability,
    },
    /// Odmawiaj całej rodziny zdolności (`fs.write`, `net.egress`…).
    DenyFamily {
        /// Rodzina.
        family: String,
    },
    /// Ogranicz do podanych zdolności (każda musi mieścić się w suficie).
    RestrictTo {
        /// Zdolności.
        capabilities: Vec<Capability>,
    },
    /// Każde użycie rodziny wymaga zatwierdzenia w Broker-UI.
    RequireApproval {
        /// Rodzina.
        family: String,
    },
    /// Sufit poziomu autonomii (≤ bieżący).
    CapAutonomy {
        /// Poziom.
        max: AutonomyLevel,
    },
    /// Sufit budżetów zadań (każdy ≤ bieżący).
    CapBudget {
        /// Kroki.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_steps: Option<u32>,
        /// Czas (ms).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_wall_ms: Option<u64>,
        /// Koszt (mikro-PLN).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_cost_micro_pln: Option<u64>,
    },
    /// Najwięcej zadań agentek naraz (≤ bieżący limit).
    MaxParallel {
        /// Limit.
        n: u32,
    },
    /// Okno ciszy dla wyzwalaczy.
    QuietHours {
        /// Początek (minuta doby).
        start_min: u16,
        /// Koniec (minuta doby).
        end_min: u16,
    },
    /// Żadnych mostów CLI (także w harmonogramach).
    DenyBridges,
}

impl Effect {
    /// Krótka nazwa.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Exclusive { .. } => "exclusive",
            Self::Preempt { .. } => "preempt",
            Self::PauseAtAtomic { .. } => "pause_at_atomic",
            Self::DenyCapability { .. } => "deny_capability",
            Self::DenyFamily { .. } => "deny_family",
            Self::RestrictTo { .. } => "restrict_to",
            Self::RequireApproval { .. } => "require_approval",
            Self::CapAutonomy { .. } => "cap_autonomy",
            Self::CapBudget { .. } => "cap_budget",
            Self::MaxParallel { .. } => "max_parallel",
            Self::QuietHours { .. } => "quiet_hours",
            Self::DenyBridges => "deny_bridges",
        }
    }
}

/// Reguła.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Identyfikator.
    pub id: RuleId,
    /// Opis (zwykły tekst; nie ma mocy sprawczej).
    #[serde(default)]
    pub description: String,
    /// Warunek.
    #[serde(default)]
    pub when: When,
    /// Efekty.
    pub then: Vec<Effect>,
}

/// Parsuje szkic reguły (JSON z LLM albo edytora) — ścisłe pola, bez nieznanych kluczy.
pub fn parse_rule(draft: &serde_json::Value) -> Result<Rule, String> {
    serde_json::from_value(draft.clone()).map_err(|e| format!("niepoprawna reguła: {e}"))
}
