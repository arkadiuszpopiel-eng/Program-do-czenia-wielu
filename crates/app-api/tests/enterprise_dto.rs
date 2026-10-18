//! Konto firmowe (C2.12) na łączu: `accounts_add` przyjmuje ustawienia opcjonalnie (stary interfejs
//! bez pola działa), konto oddaje je tylko wtedy, gdy są ustawione.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_api::dto::{Account, AddAccountInput, AzureDeployment, EnterpriseSettings};
use serde_json::json;

#[test]
fn add_input_enterprise_is_optional() {
    let plain: AddAccountInput = serde_json::from_value(json!({
        "provider_id": "openai", "label": "", "secret": "sk-x", "base_url": null
    }))
    .unwrap();
    assert!(plain.enterprise.is_empty());
    let azure: AddAccountInput = serde_json::from_value(json!({
        "provider_id": "azure-openai", "label": "Azure", "secret": "k", "base_url": "https://c.openai.azure.com",
        "enterprise": {"api_version": "2024-10-21", "deployments": [{"name": "prod-4o", "model": "gpt-4o"}]}
    }))
    .unwrap();
    assert_eq!(
        azure.enterprise.deployments,
        vec![AzureDeployment {
            name: "prod-4o".into(),
            model: Some("gpt-4o".into())
        }]
    );
    assert!(
        !format!("{azure:?}").contains("\"k\""),
        "sekret nie w Debug"
    );
}

#[test]
fn account_dto_carries_enterprise_only_when_set() {
    let base = json!({
        "id": "acc-1", "provider_id": "openai", "label": "x", "state": "ok", "key_stored": true,
        "models": [], "assignments": {"task_classes": [], "agents": [], "voice_stt": false, "voice_tts": false},
        "cost_limit": {"enabled": false, "monthly": {"minor": 0, "currency": "PLN"}},
        "last_tested_at": null
    });
    let account: Account = serde_json::from_value(base.clone()).unwrap();
    assert_eq!(account.enterprise, EnterpriseSettings::default());
    assert_eq!(serde_json::to_value(&account).unwrap(), base);
    let mut with = base;
    with["enterprise"] = json!({"organization": "org-Acme", "project": "proj_B"});
    let account: Account = serde_json::from_value(with.clone()).unwrap();
    assert_eq!(account.enterprise.project.as_deref(), Some("proj_B"));
    assert_eq!(serde_json::to_value(&account).unwrap(), with);
}
