//! Kontrakt klasyfikatora ryzyka (docs/modules/risk-classifier/SPEC.md, PLAN §8.3, §8.7, §6.10).
//!
//! Klasyfikator jest deterministyczny i bez LLM: fakty o akcji ([`ActionFacts`]) + poziom
//! autonomii ([`AutonomyLevel`]) → [`RiskVerdict`] (klasa ryzyka, werdykt, reguły, wyjaśnienie).
//! Czysta logika ([`assess`], [`evaluate`], tabela [`RULES`]) mieszka tutaj, żeby `-impl`
//! i `-fake` nie mogły się rozjechać. Twarde reguły Jądra ([`KernelRule`]) obowiązują na każdym
//! poziomie, także L4; progi ([`RiskPolicy`]) są polityką Jądra — zmienia je tylko Broker.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod assess;
mod facts;
mod rules;
mod types;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use assess::{RiskVerdict, Verdict, assess, evaluate};
pub use facts::ActionFacts;
pub use rules::{RULES, RiskPolicy, RuleDescription, RuleId, describe};
pub use types::{
    ActionClass, AutonomyLevel, CommandOrigin, Destructiveness, KernelRule, Reversibility,
    RiskLevel, ScopeRelation, SttConfidence,
};

use core_bus_contract::EventKind;

/// Zdarzenie (Audyt przez Brokera): klasyfikacja zakończona `Ask`/`HardBlock`.
pub const EVENT_RISK_CLASSIFIED: &str = "risk.classified";
/// Zdarzenie: wykryto „lethal trifecta”.
pub const EVENT_TRIFECTA_DETECTED: &str = "risk.trifecta_detected";
/// Zdarzenie: zmieniono progi/reguły (tylko Broker).
pub const EVENT_RULES_CHANGED: &str = "risk.rules.changed";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Klasyfikator ryzyka. Ten sam `ActionFacts` + poziom = ten sam werdykt.
pub trait RiskClassifier: Send + Sync {
    /// Bieżące progi.
    fn policy(&self) -> RiskPolicy;

    /// Werdykt dla akcji na danym poziomie autonomii.
    fn evaluate(&self, facts: &ActionFacts, autonomy: AutonomyLevel) -> RiskVerdict;

    /// Tabela reguł (UI „dlaczego pyta”).
    fn rules(&self) -> Vec<RuleDescription> {
        RULES.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_shapes() {
        let o = CommandOrigin::UserVoice {
            confidence: SttConfidence::from_ratio(0.55),
            speaker_verified: false,
        };
        let json = serde_json::to_value(o).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"origin": "user_voice", "confidence": 550, "speaker_verified": false})
        );
        assert!(serde_json::from_str::<SttConfidence>("1001").is_err());
        let v = Verdict::Ask {
            non_voice: true,
            grantable: false,
        };
        let back: Verdict = serde_json::from_value(serde_json::to_value(v).unwrap()).unwrap();
        assert_eq!(back, v);
        assert_eq!(
            serde_json::to_value(Reversibility::Scoped).unwrap(),
            serde_json::json!("scoped")
        );
    }

    #[test]
    fn stt_ratio_conversion() {
        assert_eq!(SttConfidence::from_ratio(0.55).permille(), 550);
        assert_eq!(SttConfidence::from_ratio(f32::NAN).permille(), 0);
        assert_eq!(SttConfidence::from_ratio(-1.0).permille(), 0);
        assert_eq!(SttConfidence::from_ratio(7.0).permille(), 1000);
        assert_eq!(SttConfidence::from_permille(5000).permille(), 1000);
    }

    #[test]
    fn rule_table_invariants() {
        for r in RULES {
            if r.applies_up_to.is_none() {
                assert!(
                    !r.grantable,
                    "{:?}: reguła każdego poziomu nie może być grantable",
                    r.id
                );
            }
        }
        assert_eq!(describe(RuleId::Trifecta).id, RuleId::Trifecta);
        assert!(RiskPolicy::default().validate().is_ok());
        let bad = RiskPolicy {
            stt_confidence_min: SttConfidence::from_permille(100),
            ..RiskPolicy::default()
        };
        assert!(bad.validate().is_err());
        let bad_bulk = RiskPolicy {
            bulk_threshold: 1,
            ..RiskPolicy::default()
        };
        assert!(bad_bulk.validate().is_err());
        assert_eq!(AutonomyLevel::default(), AutonomyLevel::L3);
        assert_eq!(RiskLevel::Critical.bumped(), RiskLevel::Critical);
        assert!(AutonomyLevel::L4.to_string().contains("Maks"));
    }

    #[test]
    fn explanation_mentions_rules() {
        let f = ActionFacts::new("tools-fs.delete", ActionClass::Write)
            .destructive(Destructiveness::Recoverable)
            .origin(CommandOrigin::UserVoice {
                confidence: SttConfidence::from_permille(950),
                speaker_verified: true,
            });
        let v = evaluate(&f, AutonomyLevel::L4, &RiskPolicy::default());
        assert!(
            v.explanation.starts_with("Ryzyko średnie"),
            "{}",
            v.explanation
        );
        assert!(v.explanation.contains("Destrukcja zlecona głosem"));
    }
}
