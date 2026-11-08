//! Property-based: kroki odwracalne, klasyfikator w granicach okna, planista nigdy nie dotyka
//! kluczy zakazanych i zawsze kieruje obszar Jądra do Brokera.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use diagnostician_contract::contract_tests::MiniWorld;
use diagnostician_contract::{
    ClassifierConfig, Detection, FailureKind, RepairStep, Signal, Symptom, TimedSignal, classify,
    is_forbidden_key, plan,
};
use proptest::prelude::*;
use serde_json::json;

fn step() -> impl Strategy<Value = RepairStep> {
    let s = "[a-z]{1,6}(\\.[a-z]{1,6}){0,2}";
    prop_oneof![
        (s, any::<Option<i32>>(), any::<Option<i32>>()).prop_map(|(key, o, n)| {
            RepairStep::SetConfig {
                key,
                old: o.map(|v| json!(v)),
                new: n.map(|v| json!(v)),
            }
        }),
        (s, s).prop_map(|(a, b)| RepairStep::MoveFile { from: a, to: b }),
        (s, s).prop_map(|(a, b)| RepairStep::CopyFile { from: a, to: b }),
        (s, 1u64..10_000, s).prop_map(|(a, n, b)| RepairStep::ArchiveEntries {
            store: a,
            entries: n,
            archive: b
        }),
        (s, "[0-9a-f]{64}").prop_map(|(a, h)| RepairStep::QueueDownload { item: a, sha256: h }),
        s.prop_map(|m| RepairStep::RestartModule { module: m }),
    ]
}

fn kind() -> impl Strategy<Value = FailureKind> {
    (0..FailureKind::ALL.len()).prop_map(|i| FailureKind::ALL[i])
}

fn symptom() -> impl Strategy<Value = Symptom> {
    prop_oneof![
        Just(Symptom::DbBusy),
        Just(Symptom::GpuDeviceLost),
        Just(Symptom::NetworkUnreachable),
        (prop_oneof![Just(401u16), Just(429), Just(500)])
            .prop_map(|status| Symptom::Http { status }),
        (prop_oneof![Just(5i32), Just(32), Just(112), Just(2)])
            .prop_map(|os_error| Symptom::Io { os_error }),
        any::<i64>().prop_map(|skew_ms| Symptom::ClockSkew {
            skew_ms: skew_ms / 1_000
        }),
    ]
}

proptest! {
    #[test]
    fn every_step_inverts_to_itself(s in step()) {
        prop_assert_eq!(s.inverse().inverse(), s.clone());
        prop_assert!(!s.describe().is_empty());
    }

    #[test]
    fn classifier_respects_window_and_counts(
        raw in proptest::collection::vec((0u64..2_000_000, symptom(), "[a-c]"), 0..60),
        now in 0u64..2_000_000,
    ) {
        let cfg = ClassifierConfig::default();
        let signals: Vec<TimedSignal> = raw.into_iter().map(|(ts, symptom, t)| TimedSignal {
            ts_ms: ts,
            signal: Signal::Error { module: "m".into(), symptom, target: Some(t), details: BTreeMap::new() },
        }).collect();
        for d in classify(&signals, now, &cfg) {
            prop_assert!(d.first_ms <= d.last_ms && d.last_ms <= now);
            prop_assert!(now - d.first_ms <= cfg.window_ms);
            prop_assert!(d.count >= 1);
        }
    }

    #[test]
    fn plans_are_complete_safe_and_kernel_goes_to_broker(
        k in kind(),
        target in "[a-z/._-]{1,20}",
        module in prop_oneof![Just("voice-stt"), Just("core-config"), Just("safety-broker"), Just("sessions")],
        key in prop_oneof![Just("diagnostician.autonomy"), Just("improver.x"), Just("kernel.egress"), Just("voice.stt.device"), Just("privacy.tag")],
    ) {
        let world = MiniWorld::new(&[], &[]);
        let details = ["device_key", "port_key", "model_key", "dir_key", "busy_timeout_key", "safe_mode_key", "limit_key", "enabled_key"]
            .iter().map(|d| ((*d).to_owned(), key.to_owned())).chain([("sha256".to_owned(), "0".repeat(64))]).collect();
        let d = Detection { kind: k, target, module: module.into(), evidence: vec!["x".into()], count: 3, first_ms: 0, last_ms: 0, details };
        let p = plan(&d, world.as_ref());
        prop_assert_eq!(p.diff.len(), p.steps.len());
        prop_assert_eq!(p.rollback_plan.len(), p.steps.len());
        prop_assert!(!p.rationale.is_empty());
        prop_assert!(p.steps.iter().filter_map(RepairStep::config_key).all(|k| !is_forbidden_key(k)));
        if k.kernel_only() || p.steps.iter().filter_map(RepairStep::config_key).any(|k| k.starts_with("kernel.")) {
            prop_assert!(p.kernel_area);
        }
        if p.steps.is_empty() {
            prop_assert!(p.needs_human.is_some());
        }
    }
}
