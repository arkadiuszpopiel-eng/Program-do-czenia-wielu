//! `set_enterprise` w implementacji: zdarzenie `accounts.enterprise.changed` bez pełnych
//! identyfikatorów, zapis metadanych (po restarcie zostaje), błąd zapisu niczego nie zmienia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use accounts_hub_contract::contract_tests::{KEY_OK, fixture_catalog};
use accounts_hub_contract::{
    AccountSource, AccountsError, AccountsHub, Assignments, EVENT_ENTERPRISE_CHANGED,
    EnterpriseSettings, NewAccount, ProviderCatalogEntry, ProviderId, SecretString, event_kind,
};
use accounts_hub_fake::MemorySecretStore;
use accounts_hub_impl::JsonFileRepository;
use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};

const ORG: &str = "org-acmecorporation1234";
const PROJECT: &str = "proj_budgetmarketing9876";

fn catalog() -> Vec<ProviderCatalogEntry> {
    let mut catalog = fixture_catalog();
    let acme = catalog
        .iter()
        .find(|p| p.id.as_str() == "acme")
        .cloned()
        .unwrap();
    let mut openai = acme;
    openai.id = ProviderId::new("openai").unwrap();
    catalog.push(openai);
    catalog
}

fn openai_account() -> NewAccount {
    NewAccount {
        provider: ProviderId::new("openai").unwrap(),
        label: "Firma".into(),
        secret: SecretString::from(KEY_OK),
        base_url: None,
        assignments: Assignments::default(),
        cost_limit: None,
        source: AccountSource::Manual,
        enterprise: EnterpriseSettings::default(),
    }
}

fn settings() -> EnterpriseSettings {
    EnterpriseSettings {
        organization: Some(ORG.into()),
        project: Some(PROJECT.into()),
        ..EnterpriseSettings::default()
    }
}

#[tokio::test]
async fn event_has_masked_ids_and_change_survives_restart() {
    let dir = common::temp_dir("enterprise-edit");
    let path = dir.join("accounts.json");
    let secrets = Arc::new(MemorySecretStore::new());
    let repo = Arc::new(JsonFileRepository::new(&path));
    let mut hub = common::builder(catalog(), secrets.clone())
        .repository(repo.clone())
        .build()
        .unwrap();
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(hub.manifest().id.clone(), Arc::new(bus.clone()));
    hub.start(ctx).await.unwrap();
    let id = hub.add_account(openai_account()).await.unwrap();
    hub.set_enterprise(&id, settings()).await.unwrap();

    let events = bus.recorded_of_kind(&event_kind(EVENT_ENTERPRISE_CHANGED));
    assert_eq!(events.len(), 1);
    let payload = &events[0].payload;
    assert_eq!(payload["account"], id.as_str());
    assert_eq!(payload["provider"], "openai");
    assert_eq!(payload["organization"], "org-…1234");
    assert_eq!(payload["project"], "proj_…9876");
    let dump = serde_json::to_string(payload).unwrap();
    assert!(!dump.contains(ORG) && !dump.contains(PROJECT), "{dump}");

    let reopened = common::builder(catalog(), secrets)
        .repository(repo)
        .build()
        .unwrap();
    assert_eq!(reopened.account(&id).unwrap().enterprise, settings());
}

#[tokio::test]
async fn persist_failure_leaves_account_unchanged() {
    let dir = common::temp_dir("enterprise-edit-fail");
    let path = dir.join("accounts.json");
    let repo = Arc::new(JsonFileRepository::new(&path));
    let hub = common::builder(catalog(), Arc::new(MemorySecretStore::new()))
        .repository(repo)
        .build()
        .unwrap();
    let id = hub.add_account(openai_account()).await.unwrap();
    // Katalog w miejscu pliku metadanych: zapis musi się nie udać.
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    let result = hub.set_enterprise(&id, settings()).await;
    assert!(
        matches!(result, Err(AccountsError::Persist(_))),
        "{result:?}"
    );
    assert!(hub.account(&id).unwrap().enterprise.is_empty());
}
