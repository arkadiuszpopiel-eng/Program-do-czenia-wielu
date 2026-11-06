//! Które prośby Sędzia Cieni może oddać modelowi-sędziemu (SPEC „Które prośby idą do sędziego”).

use risk_classifier_contract::{AutonomyLevel, RuleId};

/// Reguły, które sędzia może rozstrzygnąć. Każda inna reguła w werdykcie = karta właściciela.
///
/// Poza listą celowo: reguły głosu, `AdminConsent`, `Trifecta`, `TaintedEgress`, `ProtectedPath`
/// (plan 50 §4.5), `CriticalRisk`, `KernelBlock`, oraz `MutationNeedsYes`/`RiskyAtL2` (nie działają na L3).
pub const JUDGE_RULES: [RuleId; 6] = [
    RuleId::OutsideWorkdir,
    RuleId::IrreversibleOutside,
    RuleId::EgressNotAllowlisted,
    RuleId::GuiOutsideApps,
    RuleId::TaintedHighRisk,
    RuleId::UntrustedSource,
];

/// Czy prośbę z danymi regułami, na danym poziomie efektywnym, może ocenić sędzia.
///
/// `non_voice` (potwierdzenie fizyczne) zawsze wyklucza sędziego. Pusta lista reguł też — nie wiadomo, o co
/// Sędzia Cieni pyta, więc decyduje człowiek.
pub fn eligible(rules: &[RuleId], level: AutonomyLevel, non_voice: bool) -> bool {
    level == AutonomyLevel::L3
        && !non_voice
        && !rules.is_empty()
        && rules.iter().all(|r| JUDGE_RULES.contains(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_l3() {
        for level in AutonomyLevel::ALL {
            let ok = eligible(&[RuleId::OutsideWorkdir], level, false);
            assert_eq!(ok, level == AutonomyLevel::L3, "{level:?}");
        }
    }

    #[test]
    fn non_voice_and_empty_go_to_owner() {
        assert!(!eligible(
            &[RuleId::OutsideWorkdir],
            AutonomyLevel::L3,
            true
        ));
        assert!(!eligible(&[], AutonomyLevel::L3, false));
    }

    #[test]
    fn one_excluded_rule_sends_to_owner() {
        let excluded = [
            RuleId::KernelBlock,
            RuleId::VoiceDestructive,
            RuleId::VoiceLowConfidence,
            RuleId::VoiceUnverifiedRisky,
            RuleId::AdminConsent,
            RuleId::Trifecta,
            RuleId::TaintedEgress,
            RuleId::MutationNeedsYes,
            RuleId::RiskyAtL2,
            RuleId::ProtectedPath,
            RuleId::CriticalRisk,
        ];
        for rule in excluded {
            assert!(
                !eligible(&[RuleId::OutsideWorkdir, rule], AutonomyLevel::L3, false),
                "{rule:?}"
            );
        }
    }

    #[test]
    fn all_listed_rules_pass() {
        assert!(eligible(&JUDGE_RULES, AutonomyLevel::L3, false));
    }
}
