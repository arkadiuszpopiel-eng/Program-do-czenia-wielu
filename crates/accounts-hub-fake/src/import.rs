//! Import jednego klucza ze zmiennej środowiskowej (atrapa).

use accounts_hub_contract::{
    AccountSource, AccountsError, AccountsHub, Assignments, EnvImportOutcome, NewAccount,
    ProviderCatalogEntry, SecretStore, SecretString,
};

use crate::hub::FakeAccountsHub;

impl FakeAccountsHub {
    /// Dodaje konto albo rozpoznaje duplikat (porównanie w czasie stałym).
    pub(crate) async fn import_one(
        &self,
        entry: &ProviderCatalogEntry,
        var: &str,
        value: SecretString,
    ) -> Result<EnvImportOutcome, AccountsError> {
        let existing = self.accounts().into_iter().find(|a| {
            a.provider == entry.id
                && self
                    .secrets()
                    .get(&a.secret)
                    .ok()
                    .flatten()
                    .is_some_and(|s| s.ct_eq(&value))
        });
        if let Some(a) = existing {
            return Ok(EnvImportOutcome::AlreadyPresent { account: a.id });
        }
        let account = self
            .add_account(NewAccount {
                provider: entry.id.clone(),
                label: format!("Import: {var}"),
                secret: value,
                base_url: None,
                assignments: Assignments::default(),
                cost_limit: None,
                source: AccountSource::Env {
                    var: var.to_owned(),
                },
            })
            .await?;
        Ok(EnvImportOutcome::Imported { account })
    }
}
