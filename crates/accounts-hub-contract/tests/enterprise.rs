//! Konta firmowe (C2.12): format identyfikatorów, adres zasobu Azure, oczyszczanie ustawień,
//! maskowanie w `Debug` i zgodność zapisu (konto bez pola sprzed wersji 0.0.64).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use accounts_hub_contract::{
    AZURE_DEFAULT_API_VERSION, AZURE_OPENAI, Account, AzureDeployment, EnterpriseSettings, OPENAI,
    is_azure_api_version, is_deployment_name, is_openai_organization, is_openai_project, mask_id,
    validate_azure_endpoint,
};

const AZURE: &str = "https://contoso-ai.openai.azure.com";

fn deployment(name: &str, model: Option<&str>) -> AzureDeployment {
    AzureDeployment {
        name: name.into(),
        model: model.map(str::to_owned),
    }
}

#[test]
fn identifiers_are_checked_loosely() {
    assert!(is_openai_organization("org-AbC123xyz"));
    assert!(!is_openai_organization("org-"));
    assert!(!is_openai_organization("organization-1"));
    assert!(!is_openai_organization("org-ab cd"));
    assert!(!is_openai_organization(&format!("org-{}", "a".repeat(80))));
    assert!(is_openai_project("proj_AbC123"));
    assert!(is_openai_project("proj-abc"));
    assert!(!is_openai_project("proj_"));
    assert!(!is_openai_project("prj_abc"));
    assert!(!is_openai_project("proj_a/b"));
    assert!(is_azure_api_version("2024-10-21"));
    assert!(is_azure_api_version("2025-04-01-preview"));
    assert!(!is_azure_api_version("2024-1-21"));
    assert!(!is_azure_api_version("2024-10-21-beta"));
    assert!(!is_azure_api_version("latest"));
    assert!(!is_azure_api_version("2024-10-2é"));
    assert!(!is_azure_api_version("é"));
    assert!(is_deployment_name("gpt-4o.prod_1"));
    assert!(!is_deployment_name(".hidden"));
    assert!(!is_deployment_name("a/b"));
    assert!(!is_deployment_name("a?b"));
    assert!(!is_deployment_name(""));
}

#[test]
fn ids_are_masked_in_logs() {
    assert_eq!(mask_id("org-AbCdEfGh12345678"), "org-…5678");
    assert_eq!(mask_id("proj_AbCdEfGh1234wxyz"), "proj_…wxyz");
    assert_eq!(mask_id("org-short"), "org-…");
    assert_eq!(mask_id("bez-przedrostka-ale-dlugi"), "bez-…lugi");
    let settings = EnterpriseSettings {
        organization: Some("org-AbCdEfGh12345678".into()),
        project: Some("proj_SECRETPART9999".into()),
        ..EnterpriseSettings::default()
    };
    let debug = format!("{settings:?}");
    assert!(!debug.contains("AbCdEfGh"), "{debug}");
    assert!(!debug.contains("SECRETPART"), "{debug}");
    assert!(debug.contains("org-…5678"), "{debug}");
}

#[test]
fn azure_endpoint_shape() {
    for ok in [
        AZURE,
        "https://contoso-ai.openai.azure.com/",
        "https://Contoso.cognitiveservices.azure.com",
        "http://127.0.0.1:8080",
        "http://localhost:9000/",
    ] {
        assert!(validate_azure_endpoint(ok).is_ok(), "{ok}");
    }
    for bad in [
        "https://api.openai.com/v1",
        "http://contoso.openai.azure.com",
        "https://contoso.openai.azure.com/openai/deployments/x",
        "https://contoso.openai.azure.com?api-version=1",
        "https://.openai.azure.com",
        "https://evil.example/contoso.openai.azure.com",
        "https://contoso.openai.azure.com.evil.example",
        "https://contoso.openai.azure.com:8443",
        "https://user@contoso.openai.azure.com",
        "",
    ] {
        assert!(validate_azure_endpoint(bad).is_err(), "{bad}");
    }
}

#[test]
fn openai_settings_are_trimmed_and_checked() {
    let raw = EnterpriseSettings {
        organization: Some("  org-Acme123 ".into()),
        project: Some(" ".into()),
        ..EnterpriseSettings::default()
    };
    let clean = raw.clone().checked(OPENAI, None).unwrap();
    assert_eq!(clean.organization.as_deref(), Some("org-Acme123"));
    assert_eq!(clean.project, None);
    assert!(raw.clone().checked("xai", None).is_err());
    let bad = EnterpriseSettings {
        project: Some("proj".into()),
        ..EnterpriseSettings::default()
    };
    let err = bad.checked(OPENAI, None).unwrap_err();
    assert!(err.contains("proj_"), "{err}");
    let azure_on_openai = EnterpriseSettings {
        deployments: vec![deployment("gpt-4o", None)],
        ..EnterpriseSettings::default()
    };
    assert!(azure_on_openai.checked(OPENAI, None).is_err());
    assert!(
        EnterpriseSettings::default()
            .checked("anthropic", None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn azure_settings_need_endpoint_version_and_deployments() {
    let base = EnterpriseSettings {
        deployments: vec![
            deployment(" prod-4o ", Some(" gpt-4o ")),
            deployment("o3-mini", None),
        ],
        ..EnterpriseSettings::default()
    };
    let clean = base.clone().checked(AZURE_OPENAI, Some(AZURE)).unwrap();
    assert_eq!(
        clean.api_version.as_deref(),
        Some(AZURE_DEFAULT_API_VERSION)
    );
    assert_eq!(clean.azure_api_version(), AZURE_DEFAULT_API_VERSION);
    assert_eq!(
        clean.deployments,
        vec![
            deployment("prod-4o", Some("gpt-4o")),
            deployment("o3-mini", None)
        ]
    );
    assert_eq!(clean.deployments[1].alias(), "o3-mini");
    let cases: [(EnterpriseSettings, Option<&str>, &str); 7] = [
        (base.clone(), None, "https://"),
        (
            base.clone(),
            Some("https://api.openai.com/v1"),
            "openai.azure.com",
        ),
        (
            EnterpriseSettings {
                api_version: Some("v1".into()),
                ..base.clone()
            },
            Some(AZURE),
            "RRRR-MM-DD",
        ),
        (
            EnterpriseSettings::default(),
            Some(AZURE),
            "co najmniej jedno",
        ),
        (
            EnterpriseSettings {
                deployments: vec![deployment("a/b", None)],
                ..EnterpriseSettings::default()
            },
            Some(AZURE),
            "litery",
        ),
        (
            EnterpriseSettings {
                deployments: vec![deployment("x", Some("gpt 4o"))],
                ..EnterpriseSettings::default()
            },
            Some(AZURE),
            "modelu",
        ),
        (
            EnterpriseSettings {
                deployments: vec![deployment("a", Some("gpt-4o")), deployment("gpt-4o", None)],
                ..EnterpriseSettings::default()
            },
            Some(AZURE),
            "dwa wdrożenia",
        ),
    ];
    for (settings, url, needle) in cases {
        let err = settings.checked(AZURE_OPENAI, url).unwrap_err();
        assert!(err.contains(needle), "{needle}: {err}");
    }
    let too_many = EnterpriseSettings {
        deployments: (0..40)
            .map(|i| deployment(&format!("d{i}"), None))
            .collect(),
        ..EnterpriseSettings::default()
    };
    assert!(too_many.checked(AZURE_OPENAI, Some(AZURE)).is_err());
    let org_on_azure = EnterpriseSettings {
        organization: Some("org-x1".into()),
        ..base
    };
    assert!(org_on_azure.checked(AZURE_OPENAI, Some(AZURE)).is_err());
}

#[test]
fn account_json_without_enterprise_still_loads_and_roundtrips() {
    let old = serde_json::json!({
        "id": "acc-1", "provider": "openai", "label": "Praca",
        "created_at": "2026-09-30T12:00:00Z", "last_test": null,
        "state": {"state": "active"}, "secret": "alfa.accounts.acc-1",
        "base_url": null,
        "assignments": {"task_classes": [], "agents": [], "voice": []},
        "cost_limit": null, "source": {"source": "wizard"}, "models": []
    });
    let account: Account = serde_json::from_value(old.clone()).unwrap();
    assert!(account.enterprise.is_empty());
    let back = serde_json::to_value(&account).unwrap();
    assert!(
        back.get("enterprise").is_none(),
        "puste pole nie trafia do zapisu"
    );

    let mut azure = account;
    azure.enterprise = EnterpriseSettings {
        api_version: Some("2024-10-21".into()),
        deployments: vec![deployment("prod-4o", Some("gpt-4o"))],
        ..EnterpriseSettings::default()
    };
    let json = serde_json::to_value(&azure).unwrap();
    assert_eq!(
        json["enterprise"],
        serde_json::json!({"api_version": "2024-10-21", "deployments": [{"name": "prod-4o", "model": "gpt-4o"}]})
    );
    let again: Account = serde_json::from_value(json).unwrap();
    assert_eq!(again, azure);
}

/// Ta sama tabela przypadków, na której interfejs sprawdza `checkEnterprise` (kreator i atrapa).
#[test]
fn shared_cases_match_the_interface() {
    let table: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/enterprise-cases.json")).unwrap();
    let needle = |problem: &str| match problem {
        "org_only_openai" => "tylko kont OpenAI",
        "org_format" => "org-",
        "project_format" => "proj_",
        "azure_only" => "tylko kont Azure",
        "endpoint" => "https://",
        "api_version" => "RRRR-MM-DD",
        "no_deployments" => "co najmniej jedno",
        "deployment_name" => "nazwa wdrożenia",
        "model_name" => "nazwa modelu",
        "duplicate" => "dwa wdrożenia",
        other => panic!("nieznany kod {other}"),
    };
    let cases = table["cases"].as_array().unwrap();
    assert!(cases.len() >= 20);
    for case in cases {
        let settings: EnterpriseSettings =
            serde_json::from_value(case["settings"].clone()).unwrap();
        let provider = case["provider"].as_str().unwrap();
        let result = settings.checked(provider, case["base_url"].as_str());
        match case["problem"].as_str() {
            None => {
                let clean = result.unwrap_or_else(|e| panic!("{case}: {e}"));
                assert_eq!(
                    serde_json::to_value(&clean).unwrap(),
                    case["expected"],
                    "{case}"
                );
            }
            Some(problem) => {
                let err = result.err().unwrap_or_else(|| panic!("{case}: przyjęte"));
                assert!(err.contains(needle(problem)), "{case}: {err}");
            }
        }
    }
}
