//! Sprawdzanie „reguła tylko zawęża” względem sufitu ([`Ceiling`]), konflikty między regułami
//! i polityka efektywna (sufit ∩ reguły). Deterministyczne, bez LLM.

use safety_broker_contract::{AutonomyLevel, Capability};
use scheduler_contract::{MAX_STEPS, MAX_WALL_MS, Priority};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::rule::{Effect, ResourceKind, Rule, RuleId, TimeRange};

/// Rodziny zdolności Brokera.
pub const FAMILIES: [&str; 7] = [
    "fs.read",
    "fs.write",
    "shell.exec",
    "gui.control",
    "net.egress",
    "secrets.read",
    "system.admin",
];

/// Najdłuższe czekanie na zasób (jak `scheduler-lite`).
pub const MAX_WAIT_MS: u64 = 600_000;
/// Najwięcej efektów w regule.
pub const MAX_EFFECTS: usize = 16;

/// Bieżący sufit (to, na co dziś pozwala użytkownik/Broker) — reguły mogą tylko zejść niżej.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ceiling {
    /// Poziom autonomii.
    pub autonomy: AutonomyLevel,
    /// Kroki zadania.
    pub max_steps: u32,
    /// Czas zadania (ms).
    pub max_wall_ms: u64,
    /// Koszt zadania (mikro-PLN); `None` = bez limitu.
    pub max_cost_micro_pln: Option<u64>,
    /// Zdolności przyznane w sesji (sufit tokenów).
    pub capabilities: Vec<Capability>,
    /// Zadania agentek naraz.
    pub max_parallel: u32,
}

impl Default for Ceiling {
    fn default() -> Self {
        Self {
            autonomy: AutonomyLevel::L3,
            max_steps: 40,
            max_wall_ms: 15 * 60 * 1000,
            max_cost_micro_pln: None,
            capabilities: Vec::new(),
            max_parallel: 4,
        }
    }
}

/// Naruszenie reguły.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "violation", rename_all = "snake_case")]
pub enum Violation {
    /// Efekt rozszerzałby uprawnienia ponad sufit.
    Widening {
        /// Efekt.
        effect: String,
        /// Wyjaśnienie.
        detail: String,
    },
    /// Wartość poza zakresem / niepoprawna.
    Invalid {
        /// Wyjaśnienie.
        detail: String,
    },
}

fn widen(effect: &Effect, detail: impl Into<String>) -> Violation {
    Violation::Widening {
        effect: effect.name().into(),
        detail: detail.into(),
    }
}

fn invalid(detail: impl Into<String>) -> Violation {
    Violation::Invalid {
        detail: detail.into(),
    }
}

fn minutes_ok(a: u16, b: u16) -> bool {
    a < 1440 && b < 1440
}

fn check_effect(effect: &Effect, c: &Ceiling, out: &mut Vec<Violation>) {
    match effect {
        Effect::Exclusive { max_wait_ms, .. } => {
            if *max_wait_ms == 0 || *max_wait_ms > MAX_WAIT_MS {
                out.push(invalid(format!(
                    "max_wait_ms {max_wait_ms} poza 1–{MAX_WAIT_MS}"
                )));
            }
        }
        Effect::Preempt { classes } => {
            if classes.is_empty() {
                out.push(invalid("pusta lista priorytetów"));
            }
            if classes.iter().any(|p| *p > Priority::Interactive) {
                out.push(widen(
                    effect,
                    "nie wolno wywłaszczać mowy użytkownika ani komunikatów krytycznych",
                ));
            }
        }
        Effect::PauseAtAtomic { .. } | Effect::DenyBridges | Effect::DenyCapability { .. } => {}
        Effect::DenyFamily { family } | Effect::RequireApproval { family } => {
            if !FAMILIES.contains(&family.as_str()) {
                out.push(invalid(format!("nieznana rodzina zdolności `{family}`")));
            }
        }
        Effect::RestrictTo { capabilities } => {
            if capabilities.is_empty() {
                out.push(invalid("pusta lista zdolności"));
            }
            for cap in capabilities {
                if !c.capabilities.iter().any(|p| cap.is_subset_of(p)) {
                    out.push(widen(effect, format!("{cap} wykracza poza sufit")));
                }
            }
        }
        Effect::CapAutonomy { max } => {
            if *max > c.autonomy {
                out.push(widen(
                    effect,
                    format!("{max:?} ponad bieżący {:?}", c.autonomy),
                ));
            }
        }
        Effect::CapBudget {
            max_steps,
            max_wall_ms,
            max_cost_micro_pln,
        } => {
            if max_steps.is_none() && max_wall_ms.is_none() && max_cost_micro_pln.is_none() {
                out.push(invalid("pusty sufit budżetu"));
            }
            if max_steps.is_some_and(|s| s == 0 || s > c.max_steps.min(MAX_STEPS)) {
                out.push(widen(effect, "kroki ponad bieżący budżet"));
            }
            if max_wall_ms.is_some_and(|w| w == 0 || w > c.max_wall_ms.min(MAX_WALL_MS)) {
                out.push(widen(effect, "czas ponad bieżący budżet"));
            }
            if let (Some(v), Some(limit)) = (max_cost_micro_pln, c.max_cost_micro_pln)
                && *v > limit
            {
                out.push(widen(effect, "koszt ponad bieżący budżet"));
            }
        }
        Effect::MaxParallel { n } => {
            if *n == 0 || *n > c.max_parallel {
                out.push(widen(
                    effect,
                    format!("{n} zadań naraz ponad limit {}", c.max_parallel),
                ));
            }
        }
        Effect::QuietHours { start_min, end_min } => {
            if !minutes_ok(*start_min, *end_min) {
                out.push(invalid("minuty doby poza 0–1439"));
            }
        }
    }
}

/// Sprawdza regułę względem sufitu: pusta lista = reguła tylko zawęża.
pub fn check_rule(rule: &Rule, ceiling: &Ceiling) -> Vec<Violation> {
    let mut out = Vec::new();
    if !rule.id.is_valid() {
        out.push(invalid("niepoprawny identyfikator reguły"));
    }
    if rule.then.is_empty() || rule.then.len() > MAX_EFFECTS {
        out.push(invalid("reguła musi mieć 1–16 efektów"));
    }
    if rule.description.chars().count() > 500 {
        out.push(invalid("opis za długi"));
    }
    if let Some(TimeRange { start_min, end_min }) = rule.when.time
        && !minutes_ok(start_min, end_min)
    {
        out.push(invalid("pora dnia poza 0–1439"));
    }
    for effect in &rule.then {
        check_effect(effect, ceiling, &mut out);
    }
    out
}

/// Konflikt między regułami (do rozstrzygnięcia przez użytkownika).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Conflict {
    /// Pierwsza reguła.
    pub first: RuleId,
    /// Druga reguła.
    pub second: RuleId,
    /// Opis po polsku.
    pub message: String,
}

/// Konflikty w zbiorze reguł (sprzeczne zachowanie po czasie, ograniczenie do odmówionej
/// zdolności).
pub fn conflicts(rules: &[Rule]) -> Vec<Conflict> {
    let mut out = Vec::new();
    for (i, a) in rules.iter().enumerate() {
        for b in rules.iter().skip(i + 1) {
            for ea in &a.then {
                for eb in &b.then {
                    if let Some(message) = pair_conflict(ea, eb) {
                        out.push(Conflict {
                            first: a.id.clone(),
                            second: b.id.clone(),
                            message,
                        });
                    }
                }
            }
        }
    }
    out
}

fn pair_conflict(a: &Effect, b: &Effect) -> Option<String> {
    match (a, b) {
        (
            Effect::Exclusive {
                resource: ra,
                on_timeout: ta,
                ..
            },
            Effect::Exclusive {
                resource: rb,
                on_timeout: tb,
                ..
            },
        ) if ra == rb && ta != tb => Some(format!(
            "zasób {ra:?}: jedna reguła każe pytać po czasie, druga — kończyć błędem"
        )),
        (Effect::RestrictTo { capabilities }, Effect::DenyCapability { capability })
        | (Effect::DenyCapability { capability }, Effect::RestrictTo { capabilities })
            if capabilities.iter().any(|c| c.is_subset_of(capability)) =>
        {
            Some(format!(
                "ograniczenie do {capability} i jednocześnie odmowa tej zdolności"
            ))
        }
        _ => None,
    }
}

/// Rodzaj zasobu jako nazwa (UI).
pub fn resource_name(r: ResourceKind) -> &'static str {
    match r {
        ResourceKind::Speaker => "głośnik",
        ResourceKind::Mic => "mikrofon",
        ResourceKind::ScreenInput => "ekran",
        ResourceKind::File => "pliki",
    }
}
