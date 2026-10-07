//! Property-based na świecie chaosowym: dowolny ciąg kroków wykonanych z sukcesem, cofnięty
//! odwrotnościami w odwrotnej kolejności, przywraca stan 1:1.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use diagnostician_contract::{RepairEnv, RepairStep};
use diagnostician_fake::ChaosWorld;
use proptest::prelude::*;
use serde_json::json;
use watchdog_contract::ManualClock;

fn step() -> impl Strategy<Value = RepairStep> {
    let file = prop_oneof![
        Just("config/shared.toml"),
        Just("sesje/s1.db"),
        Just("kopie/sesje/s1.db"),
        Just("kwarantanna/a"),
        Just("D:/x"),
        Just("models/qwen-3b.gguf")
    ];
    let key = prop_oneof![
        Just("a.b"),
        Just("voice.stt.device"),
        Just("router.offline")
    ];
    let val = prop_oneof![
        Just(None),
        Just(Some(json!(1))),
        Just(Some(json!("cpu"))),
        Just(Some(json!(true)))
    ];
    prop_oneof![
        (key, val.clone(), val).prop_map(|(k, o, n)| RepairStep::SetConfig {
            key: k.into(),
            old: o,
            new: n
        }),
        (file.clone(), file.clone()).prop_map(|(a, b)| RepairStep::MoveFile {
            from: a.into(),
            to: b.into()
        }),
        (file.clone(), file).prop_map(|(a, b)| RepairStep::CopyFile {
            from: a.into(),
            to: b.into()
        }),
        (
            prop_oneof![Just("undo-journal"), Just("logs/diagnostics")],
            1u64..200
        )
            .prop_map(|(s, n)| RepairStep::ArchiveEntries {
                store: s.into(),
                entries: n,
                archive: "D:/archiwum/x".into()
            }),
        prop_oneof![Just("m1"), Just("m2")].prop_map(|i| RepairStep::QueueDownload {
            item: i.into(),
            sha256: "0".repeat(64)
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]
    #[test]
    fn applied_steps_revert_exactly(steps in proptest::collection::vec(step(), 1..12)) {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let world = ChaosWorld::baseline(Arc::new(ManualClock::new(0)));
            let before = world.snapshot();
            let mut receipts = Vec::new();
            for s in &steps {
                if let Ok(r) = world.apply(s).await {
                    receipts.push(r);
                }
            }
            for r in receipts.iter().rev() {
                world.apply(&r.inverse()).await.unwrap();
            }
            assert_eq!(world.snapshot(), before);
        });
    }
}
