//! Testy `FileConfigStore`: pliki, zapis atomowy, historia, schematy, przeładowanie, zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;

use core_bus_contract::EventKind;
use core_bus_contract::SessionId;
use core_bus_fake::FakeBus;
use core_config_contract::{
    ConfigError, ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope,
};
use core_config_impl::{
    ChangeSource, ConfigOptions, EVENT_INVALID, EVENT_KERNEL_POLICY_REJECTED, EVENT_RELOADED,
    FileConfigStore, MODULE_TOML, ReloadError,
};
use futures_util::{FutureExt, StreamExt};
use serde_json::json;

fn key(s: &str) -> ConfigKey {
    ConfigKey::new(s).unwrap()
}

fn open_in(dir: &Path, strict: bool) -> FileConfigStore {
    let mut options = ConfigOptions::new(dir, MachineId::new("m1"));
    options.strict_keys = strict;
    let fixed = || chrono::DateTime::<chrono::Utc>::from_timestamp(1_767_225_600, 0).unwrap();
    FileConfigStore::open(options, Arc::new(fixed)).unwrap()
}

fn schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "engine": {"type": "string", "enum": ["piper", "pocket"], "default": "piper"},
            "rate": {"type": "number", "minimum": 0.5, "maximum": 2.0}
        },
        "additionalProperties": false
    })
}

async fn user_set(s: &FileConfigStore, k: &str, v: serde_json::Value, layer: &ConfigLayer) {
    s.set(&key(k), Some(v), &Scope::Global, layer, Origin::User)
        .await
        .unwrap();
}

#[test]
fn module_toml_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "core-config");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.provides[0].to_string(), "core-config-contract@1");
}

#[tokio::test]
async fn persists_layers_and_scopes_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let s = open_in(dir.path(), false);
    user_set(
        &s,
        "voice.tts.engine",
        json!("pocket"),
        &ConfigLayer::Shared,
    )
    .await;
    let machine = ConfigLayer::Machine(MachineId::new("m1"));
    user_set(&s, "voice.tts.rate", json!(1.5), &machine).await;
    let s1 = Scope::Session(SessionId::from("s-1"));
    s.set(
        &key("ui.theme"),
        Some(json!("ciemny")),
        &s1,
        &ConfigLayer::Shared,
        Origin::User,
    )
    .await
    .unwrap();
    let shared = std::fs::read_to_string(dir.path().join("shared.toml")).unwrap();
    assert!(shared.contains("[voice.tts]") && shared.contains("engine = \"pocket\""));
    assert!(shared.contains("[\"@session\".s-1.ui]"), "{shared}");
    assert!(dir.path().join("machine/m1.toml").exists());
    let leftovers = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp-")
        })
        .count();
    assert_eq!(leftovers, 0, "brak plików tymczasowych po zapisie atomowym");
    drop(s);
    let s = open_in(dir.path(), false);
    assert_eq!(
        s.get(&key("voice.tts.rate"), &Scope::Global).await.unwrap(),
        Some(json!(1.5))
    );
    assert_eq!(
        s.get(&key("ui.theme"), &s1).await.unwrap(),
        Some(json!("ciemny"))
    );
    assert_eq!(s.get(&key("ui.theme"), &Scope::Global).await.unwrap(), None);
}

#[tokio::test]
async fn history_records_old_new_and_source() {
    let dir = tempfile::tempdir().unwrap();
    let s = open_in(dir.path(), false);
    user_set(&s, "a.x", json!(1), &ConfigLayer::Shared).await;
    user_set(&s, "a.x", json!(1), &ConfigLayer::Shared).await;
    user_set(&s, "a.y", json!(true), &ConfigLayer::Shared).await;
    s.set(
        &key("a.x"),
        None,
        &Scope::Global,
        &ConfigLayer::Shared,
        Origin::Import,
    )
    .await
    .unwrap();
    let all = s.history(None).unwrap();
    assert_eq!(all.len(), 3, "zapis tej samej wartości nie tworzy wpisu");
    let x = s.history(Some(&key("a.x"))).unwrap();
    assert_eq!((x[0].old.clone(), x[0].new.clone()), (None, Some(json!(1))));
    assert_eq!((x[1].old.clone(), x[1].new.clone()), (Some(json!(1)), None));
    assert_eq!(x[1].origin, Origin::Import);
    assert_eq!(x[1].source, ChangeSource::Api);
}

#[tokio::test]
async fn schema_validation_and_strict_keys() {
    let dir = tempfile::tempdir().unwrap();
    let s = open_in(dir.path(), true);
    s.register_schema(&key("voice.tts"), &schema()).unwrap();
    assert_eq!(
        s.get(&key("voice.tts.engine"), &Scope::Global)
            .await
            .unwrap(),
        Some(json!("piper"))
    );
    for (k, v) in [
        ("voice.tts.engine", json!("espeak")),
        ("voice.tts.rate", json!(3.0)),
        ("voice.tts.pitch", json!(1)),
    ] {
        let res = s
            .set(
                &key(k),
                Some(v),
                &Scope::Global,
                &ConfigLayer::Shared,
                Origin::User,
            )
            .await;
        assert!(
            matches!(res, Err(ConfigError::SchemaViolation { .. })),
            "{k}: {res:?}"
        );
    }
    assert!(
        !dir.path().join("shared.toml").exists(),
        "odrzucony zapis nie tworzy pliku"
    );
    user_set(&s, "voice.tts.rate", json!(1.25), &ConfigLayer::Shared).await;
    let unknown = s
        .set(
            &key("inny.klucz"),
            Some(json!(1)),
            &Scope::Global,
            &ConfigLayer::Shared,
            Origin::User,
        )
        .await;
    assert_eq!(unknown, Err(ConfigError::UnknownKey(key("inny.klucz"))));
    assert!(s.register_schema(&key("x"), &json!({"type": 5})).is_err());
}

#[tokio::test]
async fn structural_conflicts_and_bad_machine_id() {
    let dir = tempfile::tempdir().unwrap();
    let s = open_in(dir.path(), false);
    user_set(&s, "a.b", json!(1), &ConfigLayer::Shared).await;
    let deeper = s
        .set(
            &key("a.b.c"),
            Some(json!(2)),
            &Scope::Global,
            &ConfigLayer::Shared,
            Origin::User,
        )
        .await;
    assert!(matches!(deeper, Err(ConfigError::SchemaViolation { .. })));
    let bad = ConfigOptions::new(dir.path(), MachineId::new("../zła"));
    assert!(FileConfigStore::open(bad, Arc::new(chrono::Utc::now)).is_err());
}

#[tokio::test]
async fn reload_applies_external_edits_and_notifies() {
    let dir = tempfile::tempdir().unwrap();
    let bus = FakeBus::default();
    let s = open_in(dir.path(), false).with_bus(Arc::new(bus.clone()));
    user_set(&s, "voice.tts.engine", json!("piper"), &ConfigLayer::Shared).await;
    let mut watch = s.watch("voice");
    std::fs::write(
        dir.path().join("shared.toml"),
        "[voice.tts]\nengine = \"pocket\"\nrate = 1.0\n",
    )
    .unwrap();
    let changes = s.reload().await.unwrap();
    assert_eq!(changes.len(), 2);
    let first = watch.next().now_or_never().flatten().unwrap();
    assert_eq!(first.key, key("voice.tts.engine"));
    assert_eq!(
        (first.old, first.new),
        (Some(json!("piper")), Some(json!("pocket")))
    );
    let hist = s.history(Some(&key("voice.tts.engine"))).unwrap();
    assert_eq!(hist.last().unwrap().source, ChangeSource::File);
    assert_eq!(
        bus.recorded_of_kind(&EventKind::Custom(EVENT_RELOADED.into()))
            .len(),
        1
    );
    assert!(
        s.reload().await.unwrap().is_empty(),
        "bez zmian — brak zmian"
    );
}

#[tokio::test]
async fn invalid_reload_keeps_values_and_blocks_writes_until_fixed() {
    let dir = tempfile::tempdir().unwrap();
    let bus = FakeBus::default();
    let s = open_in(dir.path(), false).with_bus(Arc::new(bus.clone()));
    s.register_schema(&key("voice.tts"), &schema()).unwrap();
    user_set(
        &s,
        "voice.tts.engine",
        json!("pocket"),
        &ConfigLayer::Shared,
    )
    .await;
    let shared = dir.path().join("shared.toml");
    let cases = [
        ("[voice.tts\nengine = ", "invalid"),
        ("[providers.x]\napi_key = \"sk-abc\"\n", "invalid"),
        ("[kernel.egress]\nallow_all = true\n", "kernel"),
        ("[voice.tts]\nengine = \"espeak\"\n", "schema"),
    ];
    for (text, kind) in cases {
        std::fs::write(&shared, text).unwrap();
        let err = s.reload().await.unwrap_err();
        let got = match err {
            ReloadError::Invalid { .. } => "invalid",
            ReloadError::KernelPolicy(_) => "kernel",
            ReloadError::Schema { .. } => "schema",
        };
        assert_eq!(got, kind, "{text}");
        assert_eq!(
            s.get(&key("voice.tts.engine"), &Scope::Global)
                .await
                .unwrap(),
            Some(json!("pocket")),
            "wartości bez zmian"
        );
        let blocked = s
            .set(
                &key("voice.tts.rate"),
                Some(json!(1)),
                &Scope::Global,
                &ConfigLayer::Shared,
                Origin::User,
            )
            .await;
        assert!(
            matches!(blocked, Err(ConfigError::Persist(_))),
            "zapis wstrzymany"
        );
    }
    assert_eq!(
        bus.recorded_of_kind(&EventKind::Custom(EVENT_INVALID.into()))
            .len(),
        4
    );
    std::fs::write(&shared, "[voice.tts]\nengine = \"piper\"\n").unwrap();
    s.reload().await.unwrap();
    user_set(&s, "voice.tts.rate", json!(1), &ConfigLayer::Shared).await;
}

#[tokio::test]
async fn broken_file_at_open_is_reported_and_protected() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("shared.toml"), "to nie jest [toml").unwrap();
    let s = open_in(dir.path(), false);
    assert_eq!(s.load_problems().len(), 1);
    let res = s
        .set(
            &key("a.b"),
            Some(json!(1)),
            &Scope::Global,
            &ConfigLayer::Shared,
            Origin::User,
        )
        .await;
    assert!(matches!(res, Err(ConfigError::Persist(_))));
    let machine = ConfigLayer::Machine(MachineId::new("m1"));
    user_set(&s, "a.b", json!(2), &machine).await;
    let text = std::fs::read_to_string(dir.path().join("shared.toml")).unwrap();
    assert_eq!(text, "to nie jest [toml", "edycja użytkownika nienadpisana");
}

#[tokio::test]
async fn kernel_rejection_is_published() {
    let dir = tempfile::tempdir().unwrap();
    let bus = FakeBus::default();
    let s = open_in(dir.path(), false).with_bus(Arc::new(bus.clone()));
    let res = s
        .set(
            &key("kernel.budget.max"),
            Some(json!(1)),
            &Scope::Global,
            &ConfigLayer::Shared,
            Origin::Improver,
        )
        .await;
    assert_eq!(
        res,
        Err(ConfigError::KernelPolicy(key("kernel.budget.max")))
    );
    let events = bus.recorded_of_kind(&EventKind::Custom(EVENT_KERNEL_POLICY_REJECTED.into()));
    assert_eq!(events[0].payload["key"], "kernel.budget.max");
    assert_eq!(events[0].payload["origin"]["origin"], "improver");
}
