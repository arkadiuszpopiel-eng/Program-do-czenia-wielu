//! Komenda `accounts_set_enterprise` (C2.12, wersja 0.0.65): zmiana ustawień firmowych istniejącego
//! konta — organizacja i projekt OpenAI, wersja API i wdrożenia Azure OpenAI (adres zasobu
//! zostaje). Reguły sprawdza hub ([`AccountsHub::set_enterprise`]); tu jest tylko przekład
//! identyfikatora i błędów na język interfejsu. Ogłoszenie zmiany konta (`AccountChanged`,
//! przebudowa tras Routera) i przekład na DTO robi `app-core`.

use accounts_hub_contract::{Account, AccountId, AccountsError, AccountsHub, EnterpriseSettings};
use app_api::AppError;

/// Pole błędu, które interfejs podświetla w formularzu „Ustawienia dla firm”.
pub const FIELD: &str = "enterprise";

/// Zapisuje ustawienia firmowe konta `account_id` i zwraca konto po zmianie.
pub async fn set_enterprise(
    hub: &dyn AccountsHub,
    account_id: &str,
    enterprise: EnterpriseSettings,
) -> Result<Account, AppError> {
    let id = AccountId::new(account_id).map_err(AppError::from)?;
    match hub.set_enterprise(&id, enterprise).await {
        Ok(()) => {}
        Err(AccountsError::InvalidInput(message)) => {
            return Err(AppError::invalid_field(FIELD, sentence(&message)));
        }
        Err(other) => return Err(other.into()),
    }
    hub.account(&id)
        .ok_or_else(|| AccountsError::UnknownAccount(id).into())
}

/// Komunikat huba jako zdanie: wielka litera na początku, kropka na końcu.
fn sentence(message: &str) -> String {
    let mut chars = message.trim().chars();
    let mut out: String = chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

#[cfg(test)]
mod tests {
    use accounts_hub_contract::{
        AccountSource, Assignments, NewAccount, ProviderCatalogEntry, ProviderId, SecretString,
    };
    use accounts_hub_fake::FakeAccountsHub;
    use app_api::ErrorCode;

    use super::*;

    const KEY_OK: &str = "sk-ok-enterprise-0001";

    fn entry(id: &str) -> ProviderCatalogEntry {
        let text = format!(
            r#"
id = "{id}"
display_name = "{id}"
kind = "chat"
auth = "api_key"
base_url = "https://api.{id}.example/v1"
compat = "openai"
privacy_tag = "eu"
jurisdiction = "EU"
terms_url = "https://{id}.example/terms"
compliance_status = "green"
notes = "fixture"
[capabilities]
vision = true
tools = true
streaming = "unknown"
long_context = false
[pricing]
"#
        );
        ProviderCatalogEntry::from_toml(&text, id).unwrap()
    }

    fn hub_with_openai() -> FakeAccountsHub {
        FakeAccountsHub::new(vec![entry("acme"), entry("openai")])
    }

    async fn add(hub: &FakeAccountsHub, provider: &str) -> AccountId {
        hub.add_account(NewAccount {
            provider: ProviderId::new(provider).unwrap(),
            label: "Firma".into(),
            secret: SecretString::from(KEY_OK),
            base_url: None,
            assignments: Assignments::default(),
            cost_limit: None,
            source: AccountSource::Manual,
            enterprise: EnterpriseSettings::default(),
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn saves_and_returns_account_after_change() {
        let hub = hub_with_openai();
        let id = add(&hub, "openai").await;
        let settings = EnterpriseSettings {
            organization: Some(" org-acme1234 ".into()),
            project: Some("proj_budget".into()),
            ..EnterpriseSettings::default()
        };
        let account = set_enterprise(&hub, id.as_str(), settings).await.unwrap();
        assert_eq!(account.id, id);
        assert_eq!(
            account.enterprise.organization.as_deref(),
            Some("org-acme1234")
        );
        assert_eq!(hub.account(&id).unwrap().enterprise, account.enterprise);
    }

    #[tokio::test]
    async fn invalid_settings_become_a_sentence_on_the_enterprise_field() {
        let hub = hub_with_openai();
        let id = add(&hub, "acme").await;
        let settings = EnterpriseSettings {
            organization: Some("org-acme1234".into()),
            ..EnterpriseSettings::default()
        };
        let error = set_enterprise(&hub, id.as_str(), settings)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(
            error.message,
            "Organizacja i projekt dotyczą tylko kont OpenAI."
        );
        assert_eq!(error.details.text("field"), Some(FIELD));
    }

    #[tokio::test]
    async fn unknown_and_malformed_account_ids_are_refused() {
        let hub = hub_with_openai();
        let missing = set_enterprise(&hub, "acc-nope", EnterpriseSettings::default()).await;
        assert_eq!(missing.unwrap_err().code, ErrorCode::NotFound);
        let malformed = set_enterprise(&hub, "", EnterpriseSettings::default()).await;
        assert_eq!(malformed.unwrap_err().code, ErrorCode::InvalidInput);
    }

    #[test]
    fn sentences_keep_existing_punctuation() {
        assert_eq!(sentence("już."), "Już.");
        assert_eq!(sentence(" źle "), "Źle.");
        assert_eq!(sentence(""), ".");
    }
}
