//! F5-09 / ACC-F5-marshal-01: reguły Marszałka tylko zawężają — 0 rozszerzających przechodzi
//! (50 reguł, w tym złośliwe: `evals/F5/marshal-rules.json`) + właściwość: polityka efektywna
//! dowolnego zbioru reguł nigdy nie jest szersza niż sufit.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use marshal_contract::{
    Approver, Ceiling, Effect, Marshal, PauseScope, ResourceKind, Rule, RuleId, When, check_rule,
    compose, within,
};
use marshal_fake::FakeMarshal;
use proptest::prelude::*;
use safety_broker_contract::{AutonomyLevel, Capability};

#[derive(serde::Deserialize)]
struct Entry {
    n: u32,
    class: String,
    rule: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct Set {
    ceiling: Ceiling,
    rules: Vec<Entry>,
}

#[test]
fn zero_widening_rules_pass_50() {
    let set: Set =
        serde_json::from_str(include_str!("../../../evals/F5/marshal-rules.json")).unwrap();
    assert_eq!(set.rules.len(), 50);
    let m = FakeMarshal::new(1_790_841_600_000);
    m.core().set_ceiling(set.ceiling.clone());
    let (mut widening_ok, mut invalid_ok, mut narrowing_bad) = (0, 0, Vec::new());
    for e in &set.rules {
        let p = m
            .core()
            .propose_drafts("reguła z zestawu F5-09", vec![e.rule.clone()]);
        let accepted = p.rules.len() == 1;
        match (e.class.as_str(), accepted) {
            ("widening", true) => widening_ok += 1,
            ("invalid", true) => invalid_ok += 1,
            ("narrowing", false) => narrowing_bad.push((e.n, p.rejected[0].errors.clone())),
            _ => {}
        }
        if accepted {
            m.core().approve(p.id, Approver::UserInterface).unwrap();
        }
    }
    eprintln!(
        "F5-09: rozszerzające przyjęte {widening_ok}/22, niepoprawne przyjęte {invalid_ok}/7, zawężające odrzucone {}/21",
        narrowing_bad.len()
    );
    assert_eq!(widening_ok, 0);
    assert_eq!(invalid_ok, 0);
    assert!(narrowing_bad.is_empty(), "{narrowing_bad:?}");
    assert_eq!(m.core().rules().len(), 21);
    let eff = m.core().effective();
    assert!(within(&eff, &set.ceiling));
    assert_eq!(eff.autonomy, AutonomyLevel::L1);
    assert!(eff.bridges_denied);
}

fn caps() -> Vec<Capability> {
    let tree = |p: &str| serde_json::json!({"path": p, "subtree": true});
    [
        serde_json::json!({"cap": "fs.write", "scope": tree("c:\\users\\ja\\downloads")}),
        serde_json::json!({"cap": "fs.write", "scope": tree("c:\\users\\ja\\downloads\\a")}),
        serde_json::json!({"cap": "fs.write", "scope": tree("c:\\")}),
        serde_json::json!({"cap": "fs.read", "scope": tree("c:\\users\\ja")}),
        serde_json::json!({"cap": "net.egress", "scope": "*.example.com"}),
        serde_json::json!({"cap": "net.egress", "scope": "evil.org"}),
    ]
    .into_iter()
    .map(|v| serde_json::from_value(v).unwrap())
    .collect()
}

fn level() -> impl Strategy<Value = AutonomyLevel> {
    proptest::sample::select(AutonomyLevel::ALL.to_vec())
}

fn effect() -> impl Strategy<Value = Effect> {
    let cap = proptest::sample::select(caps());
    prop_oneof![
        level().prop_map(|max| Effect::CapAutonomy { max }),
        (
            proptest::option::of(0u32..100),
            proptest::option::of(0u64..2_000_000)
        )
            .prop_map(|(s, w)| Effect::CapBudget {
                max_steps: s,
                max_wall_ms: w,
                max_cost_micro_pln: None
            }),
        (0u32..10).prop_map(|n| Effect::MaxParallel { n }),
        proptest::collection::vec(cap.clone(), 1..3)
            .prop_map(|capabilities| Effect::RestrictTo { capabilities }),
        cap.prop_map(|capability| Effect::DenyCapability { capability }),
        Just(Effect::DenyBridges),
        Just(Effect::PauseAtAtomic {
            scope: PauseScope::Audio,
            resume_after: None
        }),
        (1u64..1_000_000).prop_map(|w| Effect::Exclusive {
            resource: ResourceKind::ScreenInput,
            max_wait_ms: w,
            on_timeout: scheduler_contract::OnTimeout::AskUser
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 500, failure_persistence: None, ..ProptestConfig::default() })]

    /// Dowolny zbiór reguł (także rozszerzających — `compose` je pomija) daje politykę w suficie.
    #[test]
    fn effective_policy_never_exceeds_ceiling(effects in proptest::collection::vec(effect(), 1..8), top in level()) {
        let ceiling = Ceiling {
            autonomy: top,
            capabilities: caps().into_iter().take(1).chain(caps().into_iter().skip(3).take(2)).collect(),
            ..Ceiling::default()
        };
        let rules: Vec<Rule> = effects
            .into_iter()
            .enumerate()
            .map(|(i, e)| Rule { id: RuleId(format!("r{i}")), description: String::new(), when: When::default(), then: vec![e] })
            .collect();
        let eff = compose(&ceiling, &rules);
        prop_assert!(within(&eff, &ceiling), "{eff:?}");
        for r in &rules {
            if check_rule(r, &ceiling).is_empty() {
                let one = compose(&ceiling, std::slice::from_ref(r));
                prop_assert!(within(&one, &ceiling));
            }
        }
    }
}
