//! Implementacja traitu `AccountsHub` (operacje na kontach, import, kreator).

use accounts_hub_contract::{
    Account, AccountId, AccountSource, AccountState, AccountsError, AccountsHub, Assignments,
    AuthKind, CostLimit, EVENT_KEY_ADDED, EVENT_KEY_REMOVED, EVENT_KEY_ROTATED, EVENT_KEY_TESTED,
    EnvImport, EnvImportOutcome, EnvSource, NewAccount, PriceTable, ProviderCatalogEntry,
    ProviderId, SECRET_READER_PREFIX, SecretName, SecretString, TestSummary, Wizard, WizardError,
    WizardStep, provider_state, validate_base_url,
};
use async_trait::async_trait;
use core_bus_contract::Level;

use crate::hub::AccountsHubService;

fn invalid(msg: &str) -> AccountsError {
    AccountsError::InvalidInput(msg.to_owned())
}

/// Reguły wejścia wspólne dla dodania konta i importu.
fn check_new(entry: &ProviderCatalogEntry, new: &NewAccount) -> Result<(), AccountsError> {
    match entry.auth {
        AuthKind::OauthCli => {
            return Err(invalid("ten dostawca wymaga logowania w oficjalnym CLI"));
        }
        AuthKind::ApiKey if !new.secret.is_plausible_key() => {
            return Err(invalid("klucz jest pusty albo zawiera niedozwolone znaki"));
        }
        AuthKind::ApiKey | AuthKind::None => {}
    }
    match (&new.base_url, &entry.base_url) {
        (Some(url), _) => validate_base_url(url).map_err(AccountsError::InvalidInput),
        (None, None) => Err(invalid(
            "katalog nie zna endpointu tego dostawcy — podaj go",
        )),
        (None, Some(_)) => Ok(()),
    }
}

impl AccountsHubService {
    fn account_or_err(&self, id: &AccountId) -> Result<Account, AccountsError> {
        self.accounts_read()
            .get(id)
            .cloned()
            .ok_or_else(|| AccountsError::UnknownAccount(id.clone()))
    }

    /// Zapisuje zmienione konto, utrwala metadane i publikuje zmianę stanu.
    async fn commit(&self, acc: Account, from: &AccountState) -> Result<(), AccountsError> {
        self.accounts_write().insert(acc.id.clone(), acc.clone());
        self.persist()?;
        self.publish_state(&acc, from).await;
        Ok(())
    }

    fn secret_of(&self, acc: &Account) -> Result<Option<SecretString>, AccountsError> {
        Ok(self.secrets.get(&acc.secret)?)
    }
}

#[async_trait]
impl AccountsHub for AccountsHubService {
    fn catalog(&self) -> Vec<ProviderCatalogEntry> {
        let ids: Vec<ProviderId> = self.catalog_read().keys().cloned().collect();
        ids.iter().filter_map(|id| self.provider(id)).collect()
    }

    fn provider(&self, id: &ProviderId) -> Option<ProviderCatalogEntry> {
        let mut entry = self.catalog_read().get(id).cloned()?;
        for acc in self.accounts_read().values().filter(|a| &a.provider == id) {
            for model in &acc.models {
                if !entry.models.iter().any(|m| m.id == model.id) {
                    entry.models.push(model.clone());
                }
            }
        }
        Some(entry)
    }

    fn provider_state(&self, id: &ProviderId) -> AccountState {
        provider_state(self.accounts_read().values().filter(|a| &a.provider == id))
    }

    fn accounts(&self) -> Vec<Account> {
        self.accounts_read().values().cloned().collect()
    }

    fn account(&self, id: &AccountId) -> Option<Account> {
        self.accounts_read().get(id).cloned()
    }

    fn set_pricing(&self, id: &ProviderId, pricing: PriceTable) -> Result<(), AccountsError> {
        let mut catalog = self.catalog_write();
        let entry = catalog
            .get_mut(id)
            .ok_or_else(|| AccountsError::UnknownProvider(id.clone()))?;
        entry.pricing = pricing;
        Ok(())
    }

    async fn add_account(&self, new: NewAccount) -> Result<AccountId, AccountsError> {
        let entry = self.provider_entry(&new.provider)?;
        self.ensure_allowed(&entry)?;
        check_new(&entry, &new)?;
        let id = self.new_id()?;
        let secret = SecretName::for_account(&id);
        if entry.auth == AuthKind::ApiKey {
            self.secrets.put(&secret, &new.secret)?;
        }
        let label = match new.label.trim() {
            "" => entry.display_name.clone(),
            l => l.to_owned(),
        };
        let acc = Account {
            id: id.clone(),
            provider: new.provider,
            label,
            created_at: self.now(),
            last_test: None,
            state: AccountState::Active,
            secret,
            base_url: new.base_url,
            assignments: new.assignments,
            cost_limit: new.cost_limit,
            source: new.source,
            models: Vec::new(),
        };
        let payload = serde_json::json!({
            "account": acc.id, "provider": acc.provider, "label": acc.label, "source": acc.source,
        });
        self.commit(acc, &AccountState::Unconfigured).await?;
        self.publish(EVENT_KEY_ADDED, Level::Info, payload).await;
        Ok(id)
    }

    async fn test_account(&self, id: &AccountId) -> Result<TestSummary, AccountsError> {
        let mut acc = self.account_or_err(id)?;
        let entry = self.provider_entry(&acc.provider)?;
        let from = acc.state.clone();
        let secret = self.secret_of(&acc)?;
        if entry.auth == AuthKind::ApiKey && secret.is_none() {
            acc.state = AccountState::Unconfigured;
            self.commit(acc, &from).await?;
            return Err(AccountsError::SecretMissing(id.clone()));
        }
        let base_url = acc.base_url.clone().or_else(|| entry.base_url.clone());
        let report = self
            .run_test(&entry, base_url.as_deref(), secret.as_ref())
            .await;
        let mut models_found = None;
        if report.outcome.key_works()
            && let Ok(models) = self
                .run_listing(&entry, base_url.as_deref(), secret.as_ref())
                .await
        {
            models_found = Some(models.len());
            acc.models = models;
        }
        let summary = TestSummary {
            at: self.now(),
            outcome: report.outcome.clone(),
            latency_ms: report.latency_ms,
            models_found,
        };
        acc.state = report.outcome.next_state(&acc.state);
        acc.last_test = Some(summary.clone());
        let payload = serde_json::json!({
            "account": acc.id, "provider": acc.provider, "outcome": summary.outcome.code(),
            "latency_ms": summary.latency_ms, "models": models_found,
        });
        self.commit(acc, &from).await?;
        self.publish(EVENT_KEY_TESTED, Level::Info, payload).await;
        Ok(summary)
    }

    async fn rotate(&self, id: &AccountId, secret: SecretString) -> Result<(), AccountsError> {
        let mut acc = self.account_or_err(id)?;
        if !secret.is_plausible_key() {
            return Err(invalid("klucz jest pusty albo zawiera niedozwolone znaki"));
        }
        self.secrets.put(&acc.secret, &secret)?;
        let from = acc.state.clone();
        if from != AccountState::Disabled {
            acc.state = AccountState::Active;
        }
        acc.last_test = None;
        let payload = serde_json::json!({ "account": acc.id, "provider": acc.provider });
        self.commit(acc, &from).await?;
        self.publish(EVENT_KEY_ROTATED, Level::Info, payload).await;
        Ok(())
    }

    async fn remove(&self, id: &AccountId) -> Result<(), AccountsError> {
        let acc = self
            .accounts_write()
            .remove(id)
            .ok_or_else(|| AccountsError::UnknownAccount(id.clone()))?;
        self.secrets.delete(&acc.secret)?;
        self.persist()?;
        let payload = serde_json::json!({ "account": acc.id, "provider": acc.provider });
        self.publish(EVENT_KEY_REMOVED, Level::Info, payload).await;
        Ok(())
    }

    async fn set_disabled(&self, id: &AccountId, disabled: bool) -> Result<(), AccountsError> {
        let mut acc = self.account_or_err(id)?;
        let from = acc.state.clone();
        acc.state = if disabled {
            AccountState::Disabled
        } else if self.secret_of(&acc)?.is_some()
            || self.provider_entry(&acc.provider)?.auth == AuthKind::None
        {
            AccountState::Active
        } else {
            AccountState::Unconfigured
        };
        self.commit(acc, &from).await
    }

    async fn update_settings(
        &self,
        id: &AccountId,
        label: &str,
        assignments: Assignments,
        cost_limit: Option<CostLimit>,
    ) -> Result<(), AccountsError> {
        let mut acc = self.account_or_err(id)?;
        if !label.trim().is_empty() {
            label.trim().clone_into(&mut acc.label);
        }
        acc.assignments = assignments;
        acc.cost_limit = cost_limit;
        let from = acc.state.clone();
        self.commit(acc, &from).await
    }

    async fn import_from_env(&self, env: &dyn EnvSource) -> Result<Vec<EnvImport>, AccountsError> {
        let mut report = Vec::new();
        for entry in self.catalog() {
            if entry.auth != AuthKind::ApiKey || self.ensure_allowed(&entry).is_err() {
                continue;
            }
            for var in &entry.env_vars {
                let outcome = match env.var(var).filter(SecretString::is_plausible_key) {
                    None => EnvImportOutcome::NotSet,
                    Some(value) => self.import_one(&entry, var, value).await?,
                };
                report.push(EnvImport {
                    provider: entry.id.clone(),
                    var: var.clone(),
                    outcome,
                });
            }
        }
        Ok(report)
    }

    fn resolve_secret(&self, id: &AccountId, caller: &str) -> Result<SecretString, AccountsError> {
        if !caller.starts_with(SECRET_READER_PREFIX) {
            return Err(AccountsError::NotPermitted(caller.to_owned()));
        }
        let acc = self.account_or_err(id)?;
        if acc.state == AccountState::Disabled {
            return Err(AccountsError::AccountDisabled(id.clone()));
        }
        if self.provider_entry(&acc.provider)?.auth == AuthKind::None {
            return Ok(SecretString::from(""));
        }
        self.secret_of(&acc)?
            .ok_or_else(|| AccountsError::SecretMissing(id.clone()))
    }

    async fn wizard_test(&self, wizard: &mut Wizard) -> Result<(), AccountsError> {
        let entry = wizard.provider().cloned().ok_or(WizardError::WrongStep {
            expected: WizardStep::TestConnection,
            actual: wizard.step(),
        })?;
        if wizard.step() != WizardStep::TestConnection {
            return Err(WizardError::WrongStep {
                expected: WizardStep::TestConnection,
                actual: wizard.step(),
            }
            .into());
        }
        self.ensure_allowed(&entry)?;
        let secret = wizard.secret().cloned();
        let base_url = wizard.base_url().map(str::to_owned);
        let report = self
            .run_test(&entry, base_url.as_deref(), secret.as_ref())
            .await;
        Ok(wizard.record_test(report)?)
    }

    async fn wizard_discover(&self, wizard: &mut Wizard) -> Result<(), AccountsError> {
        if wizard.step() != WizardStep::DiscoverModels {
            return Err(WizardError::WrongStep {
                expected: WizardStep::DiscoverModels,
                actual: wizard.step(),
            }
            .into());
        }
        let entry = wizard
            .provider()
            .cloned()
            .ok_or(WizardError::TestNotPassed)?;
        let secret = wizard.secret().cloned();
        let base_url = wizard.base_url().map(str::to_owned);
        let result = self
            .run_listing(&entry, base_url.as_deref(), secret.as_ref())
            .await;
        Ok(wizard.record_models(result)?)
    }

    async fn wizard_finish(&self, wizard: Wizard) -> Result<AccountId, AccountsError> {
        let outcome = wizard.finish()?;
        let id = self.add_account(outcome.account).await?;
        let mut acc = self.account_or_err(&id)?;
        let from = acc.state.clone();
        acc.models = outcome.models;
        acc.last_test = Some(TestSummary {
            at: self.now(),
            outcome: outcome.test.outcome.clone(),
            latency_ms: outcome.test.latency_ms,
            models_found: Some(acc.models.len()),
        });
        acc.state = outcome.test.outcome.next_state(&acc.state);
        self.commit(acc, &from).await?;
        Ok(id)
    }
}

impl AccountsHubService {
    async fn import_one(
        &self,
        entry: &ProviderCatalogEntry,
        var: &str,
        value: SecretString,
    ) -> Result<EnvImportOutcome, AccountsError> {
        let existing: Vec<Account> = self
            .accounts_read()
            .values()
            .filter(|a| a.provider == entry.id)
            .cloned()
            .collect();
        for acc in existing {
            if self.secret_of(&acc)?.is_some_and(|s| s.ct_eq(&value)) {
                return Ok(EnvImportOutcome::AlreadyPresent { account: acc.id });
            }
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
