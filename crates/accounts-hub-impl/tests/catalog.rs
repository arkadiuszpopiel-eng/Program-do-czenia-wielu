//! Katalog z repo: każdy plik przechodzi JSON Schema i model; spójność z rejestrem zgodności.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;

use accounts_hub_contract::{AuthKind, CatalogError};
use accounts_hub_impl::CatalogValidator;
use compliance_contract::{PrivacyTag, Registry};

const REGISTRY: &str = include_str!("../../../docs/compliance/compliance-registry.json");

#[test]
fn every_repo_catalog_file_passes_schema_and_model() {
    let validator = CatalogValidator::builtin().unwrap();
    let (entries, errors) = validator.load_dir(&common::repo_catalog_dir()).unwrap();
    assert!(errors.is_empty(), "{errors:#?}");
    let files = std::fs::read_dir(common::repo_catalog_dir())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "toml")
        })
        .count();
    assert_eq!(entries.len(), files);
    assert!(entries.len() >= 17);
    assert!(
        entries
            .iter()
            .all(|e| e.pricing.is_empty() && e.models.is_empty())
    );
    let anthropic = entries
        .iter()
        .find(|e| e.id.as_str() == "anthropic")
        .unwrap();
    assert_eq!(anthropic.env_vars, vec!["ANTHROPIC_API_KEY"]);
    assert_eq!(anthropic.auth, AuthKind::ApiKey);
}

#[test]
fn schema_rejects_what_the_model_would_miss() {
    let validator = CatalogValidator::builtin().unwrap();
    let text = std::fs::read_to_string(common::repo_catalog_dir().join("xai.toml")).unwrap();
    validator.parse_entry(&text, "xai").unwrap();
    let cases = [
        text.replace("kind = \"multi\"", "kind = \"video\""),
        text.replace("env_vars = [\"XAI_API_KEY\"]", "env_vars = [\"xai\"]"),
        text.replace("[pricing]", "[pricing]\ngrok = 1"),
        text.replace("id = \"xai\"", "id = \"X\""),
        text.replace("https://api.x.ai/v1", "http://api.x.ai/v1"),
    ];
    for bad in cases {
        let err = validator.parse_entry(&bad, "xai").unwrap_err();
        assert!(matches!(err, CatalogError::Schema { .. }), "{err}");
    }
    assert!(matches!(
        validator.parse_entry("id = ", "x"),
        Err(CatalogError::Syntax { .. })
    ));
    assert!(CatalogValidator::from_schema_str("{").is_err());
}

#[test]
fn load_dir_reports_broken_files_without_blocking_others() {
    let dir = common::temp_dir("catalog");
    let good = std::fs::read_to_string(common::repo_catalog_dir().join("openai.toml")).unwrap();
    std::fs::write(dir.join("openai.toml"), &good).unwrap();
    std::fs::write(dir.join("broken.toml"), "id = \"broken\"").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    let (entries, errors) = CatalogValidator::builtin().unwrap().load_dir(&dir).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].to_string().contains("broken.toml"));
    assert!(
        CatalogValidator::builtin()
            .unwrap()
            .load_dir(&dir.join("missing"))
            .is_err()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Każdy dostawca z katalogu (poza szablonami `custom-*`) ma wpis tagów w rejestrze zgodności,
/// a tagi katalogu nie są „łagodniejsze” niż w rejestrze.
#[test]
fn catalog_tags_consistent_with_compliance_registry() {
    let registry = Registry::from_json(REGISTRY).unwrap();
    let providers = registry.providers_by_id();
    let (entries, _) = CatalogValidator::builtin()
        .unwrap()
        .load_dir(&common::repo_catalog_dir())
        .unwrap();
    for e in entries
        .iter()
        .filter(|e| !e.id.as_str().starts_with("custom-"))
    {
        let reg = providers
            .get(e.id.as_str())
            .unwrap_or_else(|| panic!("brak `{}` w providers[] rejestru", e.id));
        let allowed: BTreeSet<PrivacyTag> = reg.privacy_tags.clone();
        assert!(
            allowed.contains(&e.privacy_tag),
            "{}: tag katalogu {} spoza rejestru {allowed:?}",
            e.id,
            e.privacy_tag
        );
        assert!(
            reg.jurisdiction.is_unknown() || reg.jurisdiction == e.jurisdiction,
            "{}: jurysdykcja {} vs {}",
            e.id,
            e.jurisdiction,
            reg.jurisdiction
        );
    }
}
