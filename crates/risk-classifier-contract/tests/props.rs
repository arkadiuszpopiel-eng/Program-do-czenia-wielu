//! Testy własności klasyfikatora (ACC-F3-risk-classifier-01/02/03): determinizm,
//! monotoniczność w poziomie autonomii i w czynnikach ryzyka, twarde reguły na każdym poziomie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use risk_classifier_contract::{
    ActionClass, ActionFacts, AutonomyLevel, CommandOrigin, Destructiveness, KernelRule,
    Reversibility, RiskPolicy, ScopeRelation, SttConfidence, Verdict, evaluate,
};

fn class() -> impl Strategy<Value = ActionClass> {
    prop_oneof![
        Just(ActionClass::Read),
        Just(ActionClass::Write),
        Just(ActionClass::Shell),
        Just(ActionClass::GuiControl),
        Just(ActionClass::Egress),
        Just(ActionClass::SecretsRead),
        Just(ActionClass::Admin),
    ]
}

fn origin() -> impl Strategy<Value = CommandOrigin> {
    prop_oneof![
        Just(CommandOrigin::UserText),
        Just(CommandOrigin::Agent),
        Just(CommandOrigin::UntrustedContent),
        (0u16..=1000, any::<bool>()).prop_map(|(c, v)| CommandOrigin::UserVoice {
            confidence: SttConfidence::from_permille(c),
            speaker_verified: v,
        }),
    ]
}

fn kernel() -> impl Strategy<Value = Option<KernelRule>> {
    prop_oneof![
        8 => Just(None),
        1 => Just(Some(KernelRule::AuditDisable)),
        1 => Just(Some(KernelRule::GuiControlOfKernelProcess)),
        1 => Just(Some(KernelRule::CredentialDenylist)),
    ]
}

prop_compose! {
    fn facts()(
        class in class(),
        reversible in prop_oneof![Just(Reversibility::Yes), Just(Reversibility::Scoped), Just(Reversibility::No)],
        scope in prop_oneof![Just(ScopeRelation::InScope), Just(ScopeRelation::AllowedApp), Just(ScopeRelation::Outside)],
        egress in proptest::option::of((Just("host.example".to_owned()), any::<bool>())),
        destructive in prop_oneof![Just(Destructiveness::None), Just(Destructiveness::Recoverable), Just(Destructiveness::Permanent)],
        bulk in 1u32..20_000,
        flags in any::<[bool; 4]>(),
        origin in origin(),
        kernel in kernel(),
    ) -> ActionFacts {
        let mut f = ActionFacts::new("tool", class)
            .reversible(reversible)
            .scope(scope)
            .destructive(destructive)
            .bulk(bulk)
            .origin(origin);
        if let Some((host, allow)) = egress {
            f = f.egress(host, allow);
        }
        f.install = flags[0];
        f.tainted = flags[1];
        f.untrusted_input_in_args = flags[2];
        f.touches_private_data = flags[3];
        f.kernel_rule = kernel;
        f
    }
}

fn strictness_all(f: &ActionFacts) -> Vec<u8> {
    let p = RiskPolicy::default();
    AutonomyLevel::ALL
        .iter()
        .map(|a| evaluate(f, *a, &p).verdict.strictness())
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn deterministic(f in facts()) {
        let p = RiskPolicy::default();
        for a in AutonomyLevel::ALL {
            prop_assert_eq!(evaluate(&f, a, &p), evaluate(&f, a, &p));
        }
    }

    #[test]
    fn higher_autonomy_never_asks_more(f in facts()) {
        let s = strictness_all(&f);
        prop_assert!(s.windows(2).all(|w| w[0] >= w[1]), "{:?}", s);
    }

    #[test]
    fn kernel_rules_block_on_every_level(f in facts()) {
        let p = RiskPolicy::default();
        if let Some(rule) = f.kernel_rule {
            for a in AutonomyLevel::ALL {
                prop_assert_eq!(evaluate(&f, a, &p).verdict, Verdict::HardBlock { rule });
            }
        }
    }

    #[test]
    fn voice_destruction_never_proceeds(f in facts()) {
        let p = RiskPolicy::default();
        if f.origin.is_voice() && f.destructive != Destructiveness::None && f.kernel_rule.is_none() {
            for a in AutonomyLevel::ALL {
                let v = evaluate(&f, a, &p).verdict;
                prop_assert!(matches!(v, Verdict::Ask { non_voice: true, grantable: false }), "{:?}", v);
            }
        }
    }

    #[test]
    fn tainted_egress_never_proceeds_even_on_l4(f in facts()) {
        let p = RiskPolicy::default();
        if f.tainted && f.is_egress() {
            for a in AutonomyLevel::ALL {
                prop_assert_ne!(evaluate(&f, a, &p).verdict, Verdict::Proceed);
            }
        }
    }

    #[test]
    fn untrusted_changes_never_proceed_up_to_l3(f in facts()) {
        let p = RiskPolicy::default();
        if f.untrusted_driven() && f.is_mutating() {
            for a in [AutonomyLevel::L0, AutonomyLevel::L1, AutonomyLevel::L2, AutonomyLevel::L3] {
                prop_assert_ne!(evaluate(&f, a, &p).verdict, Verdict::Proceed);
            }
        }
    }

    #[test]
    fn adding_risk_factors_never_relaxes(f in facts()) {
        let base = strictness_all(&f);
        let base_level = evaluate(&f, AutonomyLevel::L3, &RiskPolicy::default()).level;
        let variants = [
            ActionFacts { tainted: true, ..f.clone() },
            ActionFacts { untrusted_input_in_args: true, ..f.clone() },
            ActionFacts { touches_private_data: true, ..f.clone() },
            ActionFacts { install: true, ..f.clone() },
            ActionFacts { bulk: f.bulk.saturating_mul(100), ..f.clone() },
            ActionFacts { kernel_rule: Some(KernelRule::AuditDisable), ..f.clone() },
        ];
        for v in variants {
            let s = strictness_all(&v);
            prop_assert!(s.iter().zip(&base).all(|(a, b)| a >= b), "{:?} vs {:?}", s, base);
            let lvl = evaluate(&v, AutonomyLevel::L3, &RiskPolicy::default()).level;
            prop_assert!(lvl >= base_level);
        }
    }

    #[test]
    fn lower_stt_confidence_never_lowers_risk(f in facts(), c1 in 0u16..=1000, c2 in 0u16..=1000, verified in any::<bool>()) {
        let (lo, hi) = (c1.min(c2), c1.max(c2));
        let p = RiskPolicy::default();
        let with = |c: u16| ActionFacts {
            origin: CommandOrigin::UserVoice { confidence: SttConfidence::from_permille(c), speaker_verified: verified },
            ..f.clone()
        };
        let (a, b) = (with(lo), with(hi));
        prop_assert!(evaluate(&a, AutonomyLevel::L3, &p).level >= evaluate(&b, AutonomyLevel::L3, &p).level);
        for lvl in AutonomyLevel::ALL {
            prop_assert!(evaluate(&a, lvl, &p).verdict.strictness() >= evaluate(&b, lvl, &p).verdict.strictness());
        }
    }
}
