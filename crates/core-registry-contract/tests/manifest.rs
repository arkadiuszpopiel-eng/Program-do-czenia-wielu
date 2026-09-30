//! Testy parsera manifestu: poprawny, niepoprawne, round-trip, snapshot schematu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use core_registry_contract::{
    manifest_schema_json, Isolation, Lifecycle, ManifestError, ModuleKind, ModuleManifest,
    MANIFEST_SCHEMA_VERSION,
};

const VALID: &str = include_str!("fixtures/valid.toml");

const MINIMAL: &str = r#"
id = "echo"
version = "1.2.3"
kind = "service"
[budget]
ram_mb = 4
cpu_pct = 1
"#;

#[test]
fn valid_manifest_parses() {
    let m = ModuleManifest::parse_toml(VALID).unwrap();
    assert_eq!(m.id.as_str(), "voice-stt");
    assert_eq!(m.version, semver::Version::new(0, 1, 0));
    assert_eq!(m.kind, ModuleKind::VoiceEngine);
    assert_eq!(m.provides.len(), 1);
    assert_eq!(m.requires.len(), 3);
    assert_eq!(
        m.capabilities[0].scope.as_deref(),
        Some("%LOCALAPPDATA%/Alfa/models/**")
    );
    assert_eq!(m.lifecycle, Lifecycle::OnDemand);
    assert_eq!(m.isolation, Isolation::Process);
    assert_eq!(m.budget.vram_mb, Some(2560));
    assert_eq!(
        m.config_schema,
        Some(PathBuf::from("schemas/voice-stt.config.schema.json"))
    );
    assert_eq!(
        m.ui.unwrap().settings_page.as_deref(),
        Some("ui/settings/voice-stt")
    );
    assert_eq!(m.health.interval_s, 15);
}

#[test]
fn minimal_manifest_uses_defaults() {
    let m = ModuleManifest::parse_toml(MINIMAL).unwrap();
    assert_eq!(m.lifecycle, Lifecycle::Lazy);
    assert_eq!(m.isolation, Isolation::InProc);
    assert!(m.provides.is_empty() && m.requires.is_empty() && m.capabilities.is_empty());
    assert_eq!(m.health.check, "ping");
    assert!(m.ui.is_none() && m.config_schema.is_none());
}

#[test]
fn toml_round_trip() {
    let m = ModuleManifest::parse_toml(VALID).unwrap();
    let text = m.to_toml().unwrap();
    let back = ModuleManifest::parse_toml(&text).unwrap();
    assert_eq!(back, m);
}

fn with(replace_from: &str, replace_to: &str) -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(&MINIMAL.replacen(replace_from, replace_to, 1))
}

#[test]
fn invalid_manifests_are_rejected_without_panic() {
    assert!(matches!(
        with("\"echo\"", "\"Echo_Module\""),
        Err(ManifestError::Syntax(_))
    ));
    assert!(matches!(
        with("\"1.2.3\"", "\"1.2\""),
        Err(ManifestError::Syntax(_))
    ));
    assert!(matches!(
        with("\"service\"", "\"daemon\""),
        Err(ManifestError::Syntax(_))
    ));
    assert!(matches!(
        with("ram_mb = 4", "ram_mb = 0"),
        Err(ManifestError::InvalidBudget(_))
    ));
    assert!(matches!(
        with("cpu_pct = 1", "cpu_pct = 101"),
        Err(ManifestError::InvalidBudget(_))
    ));
    assert!(matches!(
        with("[budget]", "[budget]\nfoo = 1"),
        Err(ManifestError::Syntax(_))
    ));
    assert!(matches!(
        with("[budget]", "unknown_field = 1\n[budget]"),
        Err(ManifestError::Syntax(_))
    ));
    assert!(matches!(
        with(
            "kind = \"service\"",
            "kind = \"service\"\n[health]\ncheck = \"ping\"\ninterval_s = 0"
        ),
        Err(ManifestError::InvalidHealth(_))
    ));
    assert!(matches!(
        ModuleManifest::parse_toml("id = \"x\""),
        Err(ManifestError::Syntax(_))
    ));
    assert!(matches!(
        ModuleManifest::parse_toml("= = ="),
        Err(ManifestError::Syntax(_))
    ));
}

#[test]
fn contract_lists_are_validated() {
    let bad_ref = with(
        "kind = \"service\"",
        "kind = \"service\"\nrequires = [\"core-bus@1\"]",
    );
    assert!(matches!(bad_ref, Err(ManifestError::Syntax(_))));
    let dup = with(
        "kind = \"service\"",
        "kind = \"service\"\nrequires = [\"a-contract@1\", \"a-contract@1\"]",
    );
    assert_eq!(
        dup.unwrap_err(),
        ManifestError::Duplicate("a-contract@1".into(), "requires")
    );
    let self_dep = with(
        "kind = \"service\"",
        "kind = \"service\"\nprovides = [\"a-contract@1\"]\nrequires = [\"a-contract@1\"]",
    );
    assert_eq!(
        self_dep.unwrap_err(),
        ManifestError::SelfDependency("a-contract@1".into())
    );
    let bad_cap = with(
        "kind = \"service\"",
        "kind = \"service\"\ncapabilities = [\"fs.read(\"]",
    );
    assert!(matches!(bad_cap, Err(ManifestError::Syntax(_))));
}

/// Porównanie semantyczne (JSON); przy `UPDATE_SCHEMAS=1` zapis tylko przy faktycznej zmianie.
#[test]
fn schema_snapshot_matches_repo_file() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/schemas")
        .join(format!("module-manifest.v{MANIFEST_SCHEMA_VERSION}.json"));
    let generated = manifest_schema_json();
    let generated_value: serde_json::Value = serde_json::from_str(&generated).unwrap();
    let on_disk_value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    if std::env::var_os("UPDATE_SCHEMAS").is_some()
        && on_disk_value.as_ref() != Some(&generated_value)
    {
        std::fs::write(&path, &generated).unwrap();
    } else {
        assert_eq!(
            on_disk_value.as_ref(),
            Some(&generated_value),
            "schemat w {} nieaktualny — UPDATE_SCHEMAS=1 cargo test -p core-registry-contract",
            path.display()
        );
    }
    let value: serde_json::Value = serde_json::from_str(&generated).unwrap();
    assert_eq!(value["title"], serde_json::json!("ModuleManifest"));
    assert_eq!(value["required"].as_array().unwrap().len(), 4);
}
