//! Testy własności: dowolna sekwencja zapisów → wartości zgodne z modelem referencyjnym
//! (sesja > maszyna > wspólna) i identyczne po ponownym otwarciu z plików.
//! Plus obserwator plików (`notify`) na prawdziwym systemie plików.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::SessionId;
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_impl::{ConfigOptions, FileConfigStore, watch_files};
use futures_util::StreamExt;
use proptest::prelude::*;
use serde_json::json;

const KEYS: [&str; 3] = ["test.a", "test.b", "test.c"];

#[derive(Debug, Clone)]
struct Op {
    key: usize,
    machine: bool,
    session: bool,
    value: Option<i64>,
}

fn op() -> impl Strategy<Value = Op> {
    (
        0..KEYS.len(),
        any::<bool>(),
        any::<bool>(),
        proptest::option::of(0i64..5),
    )
        .prop_map(|(key, machine, session, value)| Op {
            key,
            machine,
            session,
            value,
        })
}

fn open(dir: &std::path::Path) -> FileConfigStore {
    let options = ConfigOptions::new(dir, MachineId::new("m1"));
    FileConfigStore::open(options, Arc::new(chrono::Utc::now)).unwrap()
}

/// Referencja: (sesyjny?, maszynowy?) → klucz → wartość.
type Reference = BTreeMap<(bool, bool), BTreeMap<usize, i64>>;

fn expected(reference: &Reference, key: usize, session_scope: bool) -> Option<i64> {
    let order: &[(bool, bool)] = if session_scope {
        &[(true, true), (true, false), (false, true), (false, false)]
    } else {
        &[(false, true), (false, false)]
    };
    order
        .iter()
        .find_map(|slot| reference.get(slot).and_then(|m| m.get(&key)).copied())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn writes_match_reference_and_survive_reopen(ops in proptest::collection::vec(op(), 1..25)) {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let store = open(dir.path());
            let session = Scope::Session(SessionId::from("s1"));
            let mut reference = Reference::new();
            for op in &ops {
                let scope = if op.session { session.clone() } else { Scope::Global };
                let layer = if op.machine {
                    ConfigLayer::Machine(MachineId::new("m1"))
                } else {
                    ConfigLayer::Shared
                };
                let key = ConfigKey::new(KEYS[op.key]).unwrap();
                store
                    .set(&key, op.value.map(|v| json!(v)), &scope, &layer, Origin::User)
                    .await
                    .unwrap();
                let slot = reference.entry((op.session, op.machine)).or_default();
                match op.value {
                    Some(v) => slot.insert(op.key, v),
                    None => slot.remove(&op.key),
                };
            }
            let reopened = open(dir.path());
            for (i, k) in KEYS.iter().enumerate() {
                let key = ConfigKey::new(*k).unwrap();
                for (scope, is_session) in [(Scope::Global, false), (session.clone(), true)] {
                    let want = expected(&reference, i, is_session).map(|v| json!(v));
                    assert_eq!(store.get(&key, &scope).await.unwrap(), want, "{k} {scope:?}");
                    assert_eq!(reopened.get(&key, &scope).await.unwrap(), want, "po otwarciu");
                }
            }
        });
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_watcher_reloads_after_external_edit() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(open(dir.path()));
    let mut changes = store.watch("voice");
    let _watch = watch_files(Arc::clone(&store), Duration::from_millis(50)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    std::fs::write(
        dir.path().join("shared.toml"),
        "[voice.tts]\nengine = \"pocket\"\n",
    )
    .unwrap();
    let change = tokio::time::timeout(Duration::from_secs(10), changes.next())
        .await
        .expect("obserwator nie przeładował konfiguracji w 10 s")
        .unwrap();
    assert_eq!(change.key, ConfigKey::new("voice.tts.engine").unwrap());
    assert_eq!(change.new, Some(json!("pocket")));
}
