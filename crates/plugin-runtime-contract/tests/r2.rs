//! Zmiana R2 Ulepszacza: klucz z listy `IMPROVABLE`, pierścień R2, nigdy automatycznie,
//! skrót zatwierdzenia Ulepszacza wiąże dokładny hash przejrzanej wersji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use improver_contract::{ChangeTarget, PlannedChange, Proposal, Ring, assess};
use plugin_runtime_contract::{
    PluginId, PluginRecord, PluginSource, PluginState, improver_key, parse_r2, r2_change,
    review_hash, samples,
};

fn record(id: &str, version: &str, n: u8) -> PluginRecord {
    let manifest = samples::manifest(id, version, &[0, 0x61, 0x73, 0x6d, n]);
    PluginRecord {
        review_hash: review_hash(&manifest).unwrap(),
        manifest,
        source: PluginSource::Improver {
            proposal: "P-1".into(),
        },
        state: PluginState::Proposed,
        approval: None,
        proposed_at_ms: 0,
        decided_at_ms: None,
    }
}

#[test]
fn r2_change_passes_improver_guard_as_ring_r2_never_auto() {
    let old = record("licznik-slow", "1.0.0", 1);
    let mut new = record("licznik-slow", "1.1.0", 2);
    new.manifest.capabilities = vec![];
    let change = r2_change(&new, Some(&old));
    assert_eq!(change.key, "plugins.licznik_slow.version");
    assert_eq!(
        change
            .from_version
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("1.0.0")
    );
    let target = ChangeTarget::Config {
        key: change.key.clone(),
        value: change.value(),
    };
    let current = r2_change(&old, None).value();
    let a = assess(&target, Some(&current)).unwrap();
    assert_eq!(a.ring, Ring::R2);
    assert!(!a.auto_eligible, "wtyczka nigdy nie wdraża się sama");

    let planned = |value: serde_json::Value| PlannedChange {
        key: change.key.clone(),
        old: Some(current.clone()),
        new: value,
        ring: a.ring,
        safety: a.safety,
    };
    let d1 = Proposal::digest_of(&[planned(change.value())]);
    let other = r2_change(&record("licznik-slow", "1.1.0", 3), Some(&old));
    let d2 = Proposal::digest_of(&[planned(other.value())]);
    assert_ne!(d1, d2, "inne bajty modułu = inny skrót zatwierdzenia");
}

#[test]
fn parse_r2_roundtrip_and_rejections() {
    let r = record("licznik-slow", "1.0.0", 1);
    let change = r2_change(&r, None);
    let (id, hash) = parse_r2(&change.key, &change.value()).unwrap();
    assert_eq!(id, PluginId::new("licznik-slow"));
    assert_eq!(hash, r.review_hash);
    assert_eq!(improver_key(&id), change.key);
    let bad = [
        ("kernel.egress.allow", change.value()),
        ("plugins.licznik-slow.version", change.value()),
        ("plugins.licznik_slow.version", serde_json::json!("1.2.0")),
        ("plugins.licznik_slow.version", serde_json::json!(42)),
        ("plugins._x.version", change.value()),
        ("plugins.licznik_slow.enabled", change.value()),
    ];
    for (key, value) in bad {
        assert!(parse_r2(key, &value).is_err(), "{key}");
    }
}
