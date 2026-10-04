//! Regresja CX-a (AGENTS.md: sekrety tylko w Windows Credential Manager — nigdy w eksporcie
//! `.alfa`): paczka sekretów ze starszej wersji → czytelna odmowa podglądu i importu; zwykła
//! paczka z sekcją `secrets.json` → sekcja pominięta z ostrzeżeniem, reszta importowana, magazyn
//! sekretów nietknięty. Eksportu sekretów nie ma w API (`Transfer` bez `export_secrets`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use accounts_hub_contract::SecretStore;
use transfer_contract::{
    Category, ContentEntry, DocumentStore, ImportOptions, ItemRef, Transfer, TransferError,
    Warning, content_sha256,
};

const OLD_KEY: &str = "sk-ant-api03-STARY-KLUCZ-Z-PACZKI";

fn secrets_json() -> Vec<u8> {
    format!(r#"[{{"name":"accounts/acc-9","value":"{OLD_KEY}"}}]"#).into_bytes()
}

fn legacy_manifest(entries: &[(&str, Vec<u8>)]) -> serde_json::Value {
    let content: Vec<ContentEntry> = entries
        .iter()
        .map(|(p, b)| ContentEntry::of(p, b))
        .collect();
    serde_json::json!({
        "schema_version": "1.0.0",
        "app_version": "0.0.1",
        "kind": "secrets",
        "created_at": "2026-09-30T12:00:00Z",
        "source_machine": { "id": common::MACHINE, "name": "x", "os": "Windows 11", "hw_class": "laptop-cuda" },
        "scope": { "keys": ["secrets"], "sessions": [], "counts": { "sessions": 0, "turns": 0, "documents": 0, "memory_entries": 0, "artifacts": 0, "secrets": 1 } },
        "content_sha256": content_sha256(&content),
        "content": content,
        "encryption": null,
        "notes": null
    })
}

fn stored_names(h: &common::H) -> Vec<String> {
    h.secrets
        .list()
        .unwrap()
        .iter()
        .map(|n| n.as_str().to_owned())
        .collect()
}

#[test]
fn legacy_secrets_package_is_refused_readably() {
    let h = common::harness();
    let entries = [("secrets.json", secrets_json())];
    let pkg = h.dir.path().join("alfa-sekrety-stara.alfa");
    common::craft(&pkg, Some(legacy_manifest(&entries)), &entries);
    for err in [
        h.transfer
            .inspect(&pkg, &ImportOptions::default())
            .unwrap_err(),
        h.transfer
            .import(&pkg, &ImportOptions::default())
            .unwrap_err(),
    ] {
        assert_eq!(err, TransferError::SecretsNotAllowed);
        let text = err.to_string();
        assert!(text.contains("starszej wersji"), "{text}");
        assert!(text.contains("Konta"), "{text}");
    }
    assert!(!stored_names(&h).contains(&"accounts/acc-9".to_owned()));
}

#[test]
fn secrets_section_in_regular_package_is_skipped() {
    let h = common::harness();
    let config = b"[ui]\ntheme = \"dark\"\n".to_vec();
    let pkg = h.dir.path().join("mieszana.alfa");
    common::craft(
        &pkg,
        None,
        &[
            ("config/common/shared.toml", config.clone()),
            ("secrets.json", secrets_json()),
        ],
    );
    let inspection = h.transfer.inspect(&pkg, &ImportOptions::default()).unwrap();
    assert!(
        inspection
            .report
            .items
            .iter()
            .all(|d| !matches!(d.item, ItemRef::Secret { .. })),
        "{:?}",
        inspection.report.items
    );
    assert!(
        inspection
            .report
            .warnings
            .contains(&Warning::SecretsSkipped)
    );
    let report = h.transfer.import(&pkg, &ImportOptions::default()).unwrap();
    assert_eq!((report.added, report.failed), (1, 0));
    assert!(!stored_names(&h).contains(&"accounts/acc-9".to_owned()));
    assert_eq!(
        h.stores[&Category::ConfigCommon]
            .read("shared.toml")
            .unwrap(),
        Some(config)
    );
}
