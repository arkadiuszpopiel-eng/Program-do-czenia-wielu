//! Zmiana ustawień firmowych istniejącego konta (`set_enterprise`, wersja 0.0.65) — te same
//! reguły co przy dodaniu konta, duplikat klucza w projekcie OpenAI i modele z wdrożeń Azure.
//! Hub musi znać dostawców `openai` i `azure-openai` (katalog z `run_all`).

use super::flows::{new_account, ok, pid};
use super::{KEY_OK, KEY_OK_2};
use crate::{
    AccountId, AccountsError, AccountsHub, AzureDeployment, EnterpriseSettings, ModelId, TaskClass,
};

/// Zasób Azure OpenAI użyty w testach.
const AZURE_URL: &str = "https://acme-res.openai.azure.com";

fn openai(organization: Option<&str>, project: Option<&str>) -> EnterpriseSettings {
    EnterpriseSettings {
        organization: organization.map(str::to_owned),
        project: project.map(str::to_owned),
        ..EnterpriseSettings::default()
    }
}

fn azure(version: Option<&str>, deployments: &[(&str, Option<&str>)]) -> EnterpriseSettings {
    EnterpriseSettings {
        api_version: version.map(str::to_owned),
        deployments: deployments
            .iter()
            .map(|(name, model)| AzureDeployment {
                name: (*name).to_owned(),
                model: model.map(str::to_owned),
            })
            .collect(),
        ..EnterpriseSettings::default()
    }
}

fn stored<H: AccountsHub>(hub: &H, id: &AccountId) -> EnterpriseSettings {
    hub.account(id)
        .map(|a| a.enterprise)
        .unwrap_or_else(|| panic!("konto {id}"))
}

fn invalid<T: std::fmt::Debug>(result: Result<T, AccountsError>, part: &str) {
    assert!(
        matches!(&result, Err(AccountsError::InvalidInput(m)) if m.contains(part)),
        "oczekiwano odmowy z „{part}”, jest {result:?}"
    );
}

/// OpenAI: organizacja i projekt ustawiane, oczyszczane, odrzucane po złym wzorze i usuwane;
/// odmowa nie zmienia konta. Wdrożenia nie dotyczą OpenAI, nieznane konto — błąd.
pub async fn set_enterprise_on_openai_account<H: AccountsHub>(hub: &H) {
    let missing = ok(AccountId::new("acc-nope"));
    assert!(matches!(
        hub.set_enterprise(&missing, EnterpriseSettings::default())
            .await,
        Err(AccountsError::UnknownAccount(_))
    ));
    let id = ok(hub.add_account(new_account("openai", KEY_OK)).await);
    let state = hub.account(&id).map(|a| a.state);
    ok(hub
        .set_enterprise(&id, openai(Some(" org-acme1234 "), Some("proj_budget_a")))
        .await);
    assert_eq!(
        stored(hub, &id),
        openai(Some("org-acme1234"), Some("proj_budget_a"))
    );
    assert_eq!(hub.account(&id).map(|a| a.state), state, "stan bez zmian");

    invalid(
        hub.set_enterprise(&id, openai(Some("acme"), None)).await,
        "org-",
    );
    invalid(
        hub.set_enterprise(&id, openai(None, Some("budget"))).await,
        "proj_",
    );
    invalid(
        hub.set_enterprise(&id, azure(None, &[("gpt-4o", None)]))
            .await,
        "Azure",
    );
    assert_eq!(
        stored(hub, &id),
        openai(Some("org-acme1234"), Some("proj_budget_a")),
        "odmowa niczego nie zmienia"
    );

    ok(hub.set_enterprise(&id, openai(Some(""), Some("  "))).await);
    assert_eq!(stored(hub, &id), EnterpriseSettings::default());

    let acme = ok(hub.add_account(new_account("acme", KEY_OK)).await);
    invalid(
        hub.set_enterprise(&acme, openai(Some("org-acme1234"), None))
            .await,
        "OpenAI",
    );
    ok(hub
        .set_enterprise(&acme, EnterpriseSettings::default())
        .await);
}

/// Ten sam klucz w tym samym projekcie to duplikat (odmowa z nazwą konta); inny projekt albo
/// inny klucz — wolno. Zapis tego samego projektu na tym samym koncie nie jest duplikatem.
pub async fn set_enterprise_respects_duplicate_key_rule<H: AccountsHub>(hub: &H) {
    let mut first = new_account("openai", KEY_OK);
    first.label = "Budżet A".into();
    first.enterprise = openai(None, Some("proj_budget_a"));
    let a = ok(hub.add_account(first).await);
    let mut second = new_account("openai", KEY_OK);
    second.enterprise = openai(None, Some("proj_budget_b"));
    let b = ok(hub.add_account(second).await);

    invalid(
        hub.set_enterprise(&b, openai(None, Some(" proj_budget_a ")))
            .await,
        "Budżet A",
    );
    assert_eq!(stored(hub, &b), openai(None, Some("proj_budget_b")));
    ok(hub
        .set_enterprise(&a, openai(Some("org-acme1234"), Some("proj_budget_a")))
        .await);
    ok(hub
        .set_enterprise(&b, openai(None, Some("proj_budget_c")))
        .await);

    let other_key = ok(hub.add_account(new_account("openai", KEY_OK_2)).await);
    ok(hub
        .set_enterprise(&other_key, openai(None, Some("proj_budget_a")))
        .await);
}

/// Azure OpenAI: wdrożenia i wersja API zmieniane (adres zasobu zostaje), lista modeli konta
/// idzie za wdrożeniami, a kolejki tracą modele bez wdrożenia. Pusta lista wdrożeń — odmowa.
pub async fn set_enterprise_on_azure_account<H: AccountsHub>(hub: &H) {
    let mut new = new_account("azure-openai", KEY_OK);
    new.base_url = Some(AZURE_URL.into());
    new.enterprise = azure(None, &[("prod-4o", Some("gpt-4o"))]);
    let id = ok(hub.add_account(new).await);
    let gpt4o = ok(ModelId::new("gpt-4o"));
    ok(hub.set_manual_model(&id, gpt4o.clone()).await);
    ok(hub
        .set_model_queue(&id, TaskClass::Conversation, vec![gpt4o])
        .await);

    let next = azure(
        Some("2025-04-01-preview"),
        &[("prod-mini", Some("gpt-4o-mini")), ("o3-mini", None)],
    );
    ok(hub.set_enterprise(&id, next).await);
    let acc = hub.account(&id).unwrap_or_else(|| panic!("konto {id}"));
    assert_eq!(acc.base_url.as_deref(), Some(AZURE_URL), "adres zostaje");
    assert_eq!(
        acc.enterprise.api_version.as_deref(),
        Some("2025-04-01-preview")
    );
    let models: Vec<&str> = acc.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(models, ["gpt-4o-mini", "o3-mini"], "modele = wdrożenia");
    assert_eq!(
        acc.models[0].display_name.as_deref(),
        Some("prod-mini"),
        "nazwa wdrożenia"
    );
    assert!(acc.queues.is_empty(), "kolejka bez wdrożenia znika");

    invalid(hub.set_enterprise(&id, azure(None, &[])).await, "wdrożenie");
    invalid(
        hub.set_enterprise(&id, azure(Some("2025"), &[("a", None)]))
            .await,
        "RRRR-MM-DD",
    );
    invalid(
        hub.set_enterprise(&id, openai(Some("org-acme1234"), None))
            .await,
        "OpenAI",
    );
    ok(hub
        .set_enterprise(&id, azure(None, &[("prod-4o", Some("gpt-4o"))]))
        .await);
    let acc = hub.account(&id).unwrap_or_else(|| panic!("konto {id}"));
    assert_eq!(
        acc.enterprise.api_version.as_deref(),
        Some(crate::AZURE_DEFAULT_API_VERSION)
    );
    assert_eq!(acc.models.len(), 1);
    assert_eq!(acc.provider, pid("azure-openai"));
}
