//! Konta firmowe (C2.12): ustawienia organizacji/projektu OpenAI i wdrożeń Azure OpenAI są
//! sprawdzane przy zapisie konta i w kreatorze — tak samo w `-impl` i w `-fake`.

use super::KEY_OK;
use super::flows::{new_account, ok};
use crate::{AccountsError, AccountsHub, AzureDeployment, EnterpriseSettings, Wizard, WizardError};

/// Organizacja, projekt i wdrożenia tylko u dostawców, których dotyczą; puste pola znikają.
pub async fn enterprise_settings_only_where_they_apply<H: AccountsHub>(hub: &H) {
    let mut org = new_account("acme", KEY_OK);
    org.enterprise.organization = Some("org-acme1234".into());
    assert!(matches!(
        hub.add_account(org).await,
        Err(AccountsError::InvalidInput(m)) if m.contains("OpenAI")
    ));
    let mut azure = new_account("acme", KEY_OK);
    azure.enterprise.deployments = vec![AzureDeployment {
        name: "gpt-4o".into(),
        model: None,
    }];
    assert!(matches!(
        hub.add_account(azure).await,
        Err(AccountsError::InvalidInput(m)) if m.contains("Azure")
    ));
    assert!(hub.accounts().is_empty(), "odrzucone konto nie powstaje");

    let mut blank = new_account("acme", KEY_OK);
    blank.enterprise.organization = Some("   ".into());
    blank.enterprise.project = Some(String::new());
    let id = ok(hub.add_account(blank).await);
    let stored = hub.account(&id).map(|a| a.enterprise).unwrap_or_default();
    assert_eq!(stored, EnterpriseSettings::default());

    let mut wizard = Wizard::new();
    let acme = hub
        .provider(&id_of(hub, &id))
        .unwrap_or_else(|| panic!("acme"));
    ok(wizard.choose_provider(acme));
    ok(wizard.enter_key(crate::SecretString::from(KEY_OK), "", None));
    let bad = EnterpriseSettings {
        project: Some("proj_acme".into()),
        ..EnterpriseSettings::default()
    };
    assert!(matches!(
        wizard.enter_enterprise(bad),
        Err(WizardError::InvalidEnterprise(_))
    ));
    ok(wizard.enter_enterprise(EnterpriseSettings::default()));
    assert!(wizard.enterprise().is_empty());
}

fn id_of<H: AccountsHub>(hub: &H, id: &crate::AccountId) -> crate::ProviderId {
    hub.account(id)
        .map(|a| a.provider)
        .unwrap_or_else(|| panic!("konto {id}"))
}
