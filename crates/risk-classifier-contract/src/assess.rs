//! Deterministyczna ocena: poziom ryzyka z faktów i werdykt dla poziomu autonomii.
//! Czysta funkcja (bez LLM, bez stanu) wspólna dla `-impl` i `-fake`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::facts::ActionFacts;
use crate::rules::{RULES, RiskPolicy, RuleId, describe};
use crate::types::{
    ActionClass, AutonomyLevel, CommandOrigin, Destructiveness, KernelRule, RiskLevel,
    ScopeRelation,
};

/// Werdykt dla akcji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// Wykonaj bez pytania.
    Proceed,
    /// Zapytaj właściciela w Broker-UI.
    Ask {
        /// Potwierdzenie musi być nie-głosem (kliknięcie/klawisz).
        non_voice: bool,
        /// Czy „zawsze zezwalaj w tym zakresie” może pokryć tę prośbę.
        grantable: bool,
    },
    /// Twarda blokada Jądra.
    HardBlock {
        /// Reguła.
        rule: KernelRule,
    },
}

impl Verdict {
    /// Surowość: 0 = wykonaj, 1 = zapytaj, 2 = blokada.
    pub fn strictness(&self) -> u8 {
        match self {
            Self::Proceed => 0,
            Self::Ask { .. } => 1,
            Self::HardBlock { .. } => 2,
        }
    }
}

/// Wynik klasyfikacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RiskVerdict {
    /// Klasa ryzyka (niezależna od poziomu autonomii).
    pub level: RiskLevel,
    /// Werdykt dla poziomu autonomii.
    pub verdict: Verdict,
    /// Reguły, które zadziałały (kolejność tabeli).
    pub rules: Vec<RuleId>,
    /// Czynniki podnoszące ryzyko (po polsku).
    pub factors: Vec<String>,
    /// Wyjaśnienie do karty zatwierdzenia (po polsku, zwykły tekst).
    pub explanation: String,
}

/// Ocena poziomu ryzyka wraz z czynnikami (po polsku).
pub fn assess(facts: &ActionFacts, policy: &RiskPolicy) -> (RiskLevel, Vec<String>) {
    let mut level = base_level(facts.class);
    let mut factors: Vec<String> = Vec::new();
    let mut raise = |min: RiskLevel, why: &str| {
        if min > level {
            level = min;
        }
        factors.push(why.to_owned());
    };
    let mutating = facts.is_mutating();
    let bulk = facts.bulk >= policy.bulk_threshold;
    match (facts.destructive, bulk) {
        (Destructiveness::None, _) => {}
        (Destructiveness::Recoverable, false) => raise(RiskLevel::Medium, "usuwanie odzyskiwalne"),
        (Destructiveness::Recoverable, true) => raise(RiskLevel::High, "masowe usuwanie"),
        (Destructiveness::Permanent, false) => raise(RiskLevel::High, "trwałe usunięcie"),
        (Destructiveness::Permanent, true) => raise(RiskLevel::Critical, "masowe trwałe usunięcie"),
    }
    if mutating && facts.effectively_irreversible() {
        if facts.scope == ScopeRelation::Outside {
            raise(RiskLevel::High, "nieodwracalne poza zakresem");
        } else {
            raise(RiskLevel::Medium, "nieodwracalne");
        }
    }
    if mutating && facts.scope == ScopeRelation::Outside {
        raise(RiskLevel::Medium, "poza zakresem sesji");
    }
    if facts.install {
        raise(RiskLevel::High, "instalacja oprogramowania");
    }
    if facts.is_egress() && facts.touches_private_data {
        raise(
            RiskLevel::High,
            "wysyłka przy dostępie do danych prywatnych",
        );
    }
    if facts.is_egress() && facts.tainted {
        raise(RiskLevel::High, "wysyłka z sesji z niezaufaną treścią");
    }
    if facts.trifecta() {
        raise(RiskLevel::Critical, "lethal trifecta");
    }
    if mutating && facts.untrusted_driven() {
        raise(RiskLevel::High, "polecenie z niezaufanej treści");
    }
    if facts.class == ActionClass::Admin && facts.destructive != Destructiveness::None {
        raise(RiskLevel::Critical, "destrukcyjna operacja administracyjna");
    }
    if facts.kernel_rule.is_some() {
        raise(RiskLevel::Critical, "obszar Jądra");
    }
    if mutating && low_confidence(facts, policy) {
        level = level.bumped();
        factors.push("niska pewność rozpoznania mowy".to_owned());
    }
    (level, factors)
}

fn base_level(class: ActionClass) -> RiskLevel {
    match class {
        ActionClass::Read | ActionClass::Write => RiskLevel::Low,
        ActionClass::Shell | ActionClass::GuiControl | ActionClass::Egress => RiskLevel::Medium,
        ActionClass::SecretsRead | ActionClass::Admin => RiskLevel::High,
    }
}

fn low_confidence(facts: &ActionFacts, policy: &RiskPolicy) -> bool {
    match facts.origin {
        CommandOrigin::UserVoice { confidence, .. } => confidence < policy.stt_confidence_min,
        _ => false,
    }
}

fn speaker_unverified(facts: &ActionFacts) -> bool {
    matches!(
        facts.origin,
        CommandOrigin::UserVoice {
            speaker_verified: false,
            ..
        }
    )
}

/// Czy warunek reguły jest spełniony (bez sprawdzania poziomu autonomii).
fn condition(id: RuleId, f: &ActionFacts, level: RiskLevel, policy: &RiskPolicy) -> bool {
    let mutating = f.is_mutating();
    let destroys = f.destructive != Destructiveness::None;
    match id {
        RuleId::KernelBlock => f.kernel_rule.is_some(),
        RuleId::VoiceDestructive => f.origin.is_voice() && destroys,
        RuleId::VoiceLowConfidence => mutating && low_confidence(f, policy),
        RuleId::VoiceUnverifiedRisky => speaker_unverified(f) && level >= RiskLevel::Medium,
        RuleId::AdminConsent => f.class == ActionClass::Admin,
        RuleId::Trifecta => f.trifecta(),
        RuleId::TaintedEgress => f.tainted && f.is_egress(),
        RuleId::MutationNeedsYes => mutating,
        RuleId::RiskyAtL2 => {
            destroys
                || f.is_egress()
                || f.install
                || matches!(f.class, ActionClass::SecretsRead | ActionClass::Admin)
                || (mutating && f.effectively_irreversible())
        }
        RuleId::TaintedHighRisk => f.tainted && level >= RiskLevel::High,
        RuleId::UntrustedSource => mutating && f.untrusted_driven(),
        RuleId::IrreversibleOutside => {
            mutating && f.effectively_irreversible() && f.scope == ScopeRelation::Outside
        }
        RuleId::CriticalRisk => level == RiskLevel::Critical,
        RuleId::EgressNotAllowlisted => f.is_egress() && !f.egress_allowlisted,
        RuleId::GuiOutsideApps => {
            f.class == ActionClass::GuiControl && f.scope == ScopeRelation::Outside
        }
    }
}

/// Werdykt dla faktów na danym poziomie autonomii.
///
/// Monotoniczność z konstrukcji: każda reguła działa „do poziomu X” albo na każdym poziomie,
/// więc wyższy poziom nigdy nie pyta o więcej niż niższy.
pub fn evaluate(facts: &ActionFacts, autonomy: AutonomyLevel, policy: &RiskPolicy) -> RiskVerdict {
    let (level, factors) = assess(facts, policy);
    if let Some(rule) = facts.kernel_rule {
        return RiskVerdict {
            level,
            verdict: Verdict::HardBlock { rule },
            rules: vec![RuleId::KernelBlock],
            explanation: format!("Zablokowane: {}.", rule.description_pl()),
            factors,
        };
    }
    let hits: Vec<_> = RULES
        .iter()
        .filter(|r| r.id != RuleId::KernelBlock)
        .filter(|r| r.applies_at(autonomy) && condition(r.id, facts, level, policy))
        .collect();
    let verdict = if hits.is_empty() {
        Verdict::Proceed
    } else {
        Verdict::Ask {
            non_voice: hits.iter().any(|r| r.non_voice),
            grantable: hits.iter().all(|r| r.grantable),
        }
    };
    let rules: Vec<RuleId> = hits.iter().map(|r| r.id).collect();
    let explanation = explain(level, &factors, &rules);
    RiskVerdict {
        level,
        verdict,
        rules,
        factors,
        explanation,
    }
}

fn explain(level: RiskLevel, factors: &[String], rules: &[RuleId]) -> String {
    let mut out = format!("Ryzyko {}", level.label_pl());
    if !factors.is_empty() {
        out.push_str(": ");
        out.push_str(&factors.join(", "));
    }
    out.push('.');
    for id in rules {
        out.push(' ');
        let text = describe(*id).description_pl;
        let mut chars = text.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
        out.push('.');
    }
    out
}
