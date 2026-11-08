//! Atrapa: operacje na kluczu istniejącego konta — te same reguły co implementacja
//! (`accounts-hub-impl/src/keys.rs`): wynik testu trafia do bieżącego wpisu, wymiana klucza zapisuje
//! nowy dopiero po teście (rozpoznany albo niesprawdzalny z góry — `accepts_new_key`; stan od nowa),
//! test bez klucza API w magazynie odmawia, a włączenie przywraca stan z ostatniego testu.

use accounts_hub_contract::{
    Account, AccountId, AccountState, AccountsError, AuthKind, ConnectionReport, ConnectionRequest,
    ConnectionTester, EnterpriseSettings, ModelId, ModelInfo, ModelLister, ProviderCatalogEntry,
    ProviderId, SecretStore, SecretString, TaskClass, TestSummary, apply_enterprise,
    duplicate_in_project,
};

use crate::hub::{FakeAccountsHub, redact};

fn invalid(message: &str) -> AccountsError {
    AccountsError::InvalidInput(message.to_owned())
}

impl FakeAccountsHub {
    async fn test_and_list(
        &self,
        entry: &ProviderCatalogEntry,
        base_url: Option<&str>,
        secret: Option<&SecretString>,
        enterprise: &EnterpriseSettings,
    ) -> (ConnectionReport, Option<Vec<ModelInfo>>) {
        let req = ConnectionRequest {
            provider: entry,
            base_url,
            secret,
            enterprise,
        };
        let report = redact(self.tester.test(req).await, secret);
        let models = if report.outcome.may_list_models() {
            let found = self.lister.list_models(req).await.ok();
            found.filter(|found| !found.is_empty())
        } else {
            None
        };
        (report, models)
    }

    async fn probe(
        &self,
        acc: &Account,
        secret: Option<&SecretString>,
    ) -> Result<(TestSummary, Option<Vec<ModelInfo>>), AccountsError> {
        let entry = self.entry(&acc.provider)?;
        let base = acc.base_url.clone().or_else(|| entry.base_url.clone());
        let (report, models) = self
            .test_and_list(&entry, base.as_deref(), secret, &acc.enterprise)
            .await;
        let summary = TestSummary {
            at: self.now(),
            outcome: report.outcome,
            latency_ms: report.latency_ms,
            models_found: models.as_ref().map(Vec::len),
            detail: report.detail,
        };
        Ok((summary, models))
    }

    /// Zapis wyniku w bieżącym wpisie; `false`, gdy konto zniknęło albo ma już inny klucz.
    fn store_test(
        &self,
        id: &AccountId,
        tested: Option<&SecretString>,
        summary: &TestSummary,
        models: Option<Vec<ModelInfo>>,
    ) -> Result<bool, AccountsError> {
        let Ok(mut current) = self.get(id) else {
            return Ok(false);
        };
        let stored = self.secrets.get(&current.secret)?;
        let same_key = match (&stored, tested) {
            (Some(a), Some(b)) => a.ct_eq(b),
            (None, None) => true,
            _ => false,
        };
        if !same_key {
            return Ok(false);
        }
        let previous = current.last_test.as_ref().map(|t| &t.outcome);
        current.state = summary.outcome.next_state_after(&current.state, previous);
        current.last_test = Some(summary.clone());
        if let Some(models) = models {
            current.models = models;
        }
        self.put(current);
        Ok(true)
    }

    pub(crate) async fn test_account_now(
        &self,
        id: &AccountId,
    ) -> Result<TestSummary, AccountsError> {
        let acc = self.get(id)?;
        let secret = self.secrets.get(&acc.secret)?;
        // Jak rdzeń (`test_account_now`): konto z kluczem API bez klucza w magazynie nie jest testowane,
        // tylko (o ile nie jest wyłączone) czeka na klucz.
        if secret.is_none() && self.entry(&acc.provider)?.auth == AuthKind::ApiKey {
            if acc.state != AccountState::Disabled {
                self.set_state(id, AccountState::Unconfigured);
            }
            return Err(AccountsError::SecretMissing(id.clone()));
        }
        let (summary, models) = self.probe(&acc, secret.as_ref()).await?;
        self.store_test(id, secret.as_ref(), &summary, models)?;
        Ok(summary)
    }

    pub(crate) async fn try_rotate_now(
        &self,
        id: &AccountId,
        secret: SecretString,
    ) -> Result<TestSummary, AccountsError> {
        let acc = self.get(id)?;
        if self.entry(&acc.provider)?.auth != AuthKind::ApiKey {
            return Err(invalid("to konto nie używa klucza API"));
        }
        if !secret.is_plausible_key() {
            return Err(invalid("klucz jest pusty albo zawiera niedozwolone znaki"));
        }
        let (summary, models) = self.probe(&acc, Some(&secret)).await?;
        if !summary.outcome.accepts_new_key() {
            return Ok(summary);
        }
        self.secrets.put(&acc.secret, &secret)?;
        // Jak `try_rotate_now` w `accounts-hub-impl`: stan ze starego klucza (brak klucza, błąd) liczony od nowa
        // od „działa”, także gdy nowego klucza nie da się sprawdzić; wyłączone zostaje wyłączone. Bieżący wpis,
        // nie kopia sprzed testu (w czasie testu konto mogło zostać wyłączone).
        if self.get(id)?.state != AccountState::Disabled {
            self.set_state(id, AccountState::Active);
        }
        self.store_test(id, Some(&secret), &summary, models)?;
        let mut saved = self.get(id)?;
        saved.key_saved_at = Some(self.now());
        self.put(saved);
        Ok(summary)
    }

    pub(crate) fn set_disabled_now(
        &self,
        id: &AccountId,
        disabled: bool,
    ) -> Result<(), AccountsError> {
        let mut acc = self.get(id)?;
        let has_key = self.secrets.get(&acc.secret)?.is_some()
            || self.entry(&acc.provider)?.auth == AuthKind::None;
        if disabled {
            acc.state = AccountState::Disabled;
        } else if acc.state == AccountState::Disabled {
            acc.state = match (&acc.last_test, has_key) {
                (_, false) => AccountState::Unconfigured,
                (Some(test), true) => test.outcome.next_state(&AccountState::Active),
                (None, true) => AccountState::Active,
            };
        }
        self.put(acc);
        Ok(())
    }

    pub(crate) fn set_manual_model_now(
        &self,
        id: &AccountId,
        model: ModelId,
    ) -> Result<(), AccountsError> {
        let mut acc = self.get(id)?;
        if acc.models.len() > 1 {
            return if acc.models.iter().any(|m| m.id == model) {
                Ok(())
            } else {
                Err(invalid(
                    "to konto ma listę modeli od dostawcy — wybierz model z listy",
                ))
            };
        }
        acc.models = vec![ModelInfo {
            id: model,
            display_name: None,
            capabilities: None,
            context_window: None,
        }];
        self.put(acc);
        Ok(())
    }

    pub(crate) fn set_model_queue_now(
        &self,
        id: &AccountId,
        class: TaskClass,
        models: Vec<ModelId>,
    ) -> Result<(), AccountsError> {
        let names: Vec<String> = models.iter().map(ToString::to_string).collect();
        accounts_hub_contract::validate_queue(class, &names)
            .map_err(AccountsError::InvalidInput)?;
        let mut acc = self.get(id)?;
        if models.is_empty() {
            acc.queues.remove(&class);
        } else {
            acc.queues.insert(class, models);
        }
        self.put(acc);
        Ok(())
    }

    /// Ustawienia firmowe istniejącego konta — te same reguły co `-impl` (bez magistrali).
    pub(crate) fn set_enterprise_now(
        &self,
        id: &AccountId,
        enterprise: EnterpriseSettings,
    ) -> Result<(), AccountsError> {
        let mut acc = self.get(id)?;
        let entry = self.entry(&acc.provider)?;
        let base_url = acc.base_url.as_deref().or(entry.base_url.as_deref());
        let checked = enterprise
            .checked(entry.id.as_str(), base_url)
            .map_err(AccountsError::InvalidInput)?;
        if let Some(secret) = self.secrets.get(&acc.secret)? {
            let url = acc.base_url.as_deref();
            let twin = self.find_same_key(&acc.provider, url, &checked, &secret);
            if let Some(other) = twin.filter(|other| other != id) {
                let label = self
                    .get(&other)
                    .map_or_else(|_| other.to_string(), |a| a.label);
                return Err(duplicate_in_project(&label));
            }
        }
        apply_enterprise(&mut acc, checked)?;
        self.put(acc);
        Ok(())
    }

    pub(crate) fn find_same_key(
        &self,
        provider: &ProviderId,
        base_url: Option<&str>,
        enterprise: &EnterpriseSettings,
        secret: &SecretString,
    ) -> Option<AccountId> {
        if secret.is_empty() {
            return None;
        }
        let default_url = self.entry(provider).ok()?.base_url;
        let effective = |url: Option<&str>| {
            url.map(str::to_owned)
                .or_else(|| default_url.clone())
                .map(|u| u.trim_end_matches('/').to_ascii_lowercase())
        };
        let wanted = effective(base_url);
        self.accounts_snapshot()
            .into_iter()
            .filter(|acc| &acc.provider == provider && effective(acc.base_url.as_deref()) == wanted)
            .filter(|acc| acc.enterprise.billing_project() == enterprise.billing_project())
            .find(|acc| {
                matches!(self.secrets.get(&acc.secret), Ok(Some(stored)) if stored.ct_eq(secret))
            })
            .map(|acc| acc.id)
    }
}
