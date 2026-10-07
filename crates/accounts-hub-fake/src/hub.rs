//! `FakeAccountsHub`: deterministyczny hub w pamięci (id `acc-1`, `acc-2`…, zegar sterowany),
//! bez magistrali i plików, z rejestrem odczytów kluczy (test szpiegowski).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use accounts_hub_contract::{
    Account, AccountId, AccountState, AccountsError, AccountsHub, Assignments, AuthKind,
    ConnectionReport, ConnectionRequest, ConnectionTester, CostLimit, EnvImport, EnvImportOutcome,
    EnvSource, ModelInfo, ModelListError, ModelLister, NewAccount, PriceTable,
    ProviderCatalogEntry, ProviderId, SECRET_READER_PREFIX, SecretName, SecretStore, SecretString,
    TestOutcome, TestSummary, Wizard, WizardError, WizardStep, provider_state, validate_base_url,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use compliance_contract::ProviderApiStatus;

use crate::ports::{MemorySecretStore, ScriptedConnectionTester, ScriptedModelLister};

#[derive(Default)]
struct State {
    catalog: BTreeMap<ProviderId, ProviderCatalogEntry>,
    accounts: BTreeMap<AccountId, Account>,
    next_id: u64,
    now: DateTime<Utc>,
    secret_reads: Vec<(AccountId, String)>,
}

/// Atrapa `AccountsHub`.
pub struct FakeAccountsHub {
    state: Mutex<State>,
    secrets: Arc<MemorySecretStore>,
    tester: Arc<ScriptedConnectionTester>,
    lister: Arc<ScriptedModelLister>,
}

fn invalid(msg: &str) -> AccountsError {
    AccountsError::InvalidInput(msg.to_owned())
}

fn redact(mut report: ConnectionReport, secret: Option<&SecretString>) -> ConnectionReport {
    if let (TestOutcome::Network { message }, Some(s)) = (&mut report.outcome, secret) {
        *message = s.redact_in(message);
    }
    report
}

impl FakeAccountsHub {
    /// Hub z katalogiem; tester i lister wg reguły prefiksu klucza.
    pub fn new(catalog: Vec<ProviderCatalogEntry>) -> Self {
        let state = State {
            catalog: catalog.into_iter().map(|p| (p.id.clone(), p)).collect(),
            ..State::default()
        };
        Self {
            state: Mutex::new(state),
            secrets: Arc::new(MemorySecretStore::new()),
            tester: Arc::new(ScriptedConnectionTester::by_key_prefix()),
            lister: Arc::new(ScriptedModelLister::by_key_prefix()),
        }
    }

    /// Magazyn sekretów atrapy.
    pub fn secrets(&self) -> &MemorySecretStore {
        &self.secrets
    }

    /// Tester (do kolejkowania raportów).
    pub fn tester(&self) -> &ScriptedConnectionTester {
        &self.tester
    }

    /// Lister modeli (do kolejkowania wyników).
    pub fn lister(&self) -> &ScriptedModelLister {
        &self.lister
    }

    /// Ustawia „teraz”.
    pub fn set_now(&self, now: DateTime<Utc>) {
        self.lock().now = now;
    }

    /// Wymusza stan konta (np. dla testów Routera); `false`, gdy konta nie ma.
    pub fn set_state(&self, id: &AccountId, state: AccountState) -> bool {
        self.lock()
            .accounts
            .get_mut(id)
            .map(|a| a.state = state)
            .is_some()
    }

    /// Kto i kiedy odczytał klucz: (konto, moduł).
    pub fn secret_reads(&self) -> Vec<(AccountId, String)> {
        self.lock().secret_reads.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn entry(&self, id: &ProviderId) -> Result<ProviderCatalogEntry, AccountsError> {
        self.lock()
            .catalog
            .get(id)
            .cloned()
            .ok_or_else(|| AccountsError::UnknownProvider(id.clone()))
    }

    fn get(&self, id: &AccountId) -> Result<Account, AccountsError> {
        self.lock()
            .accounts
            .get(id)
            .cloned()
            .ok_or_else(|| AccountsError::UnknownAccount(id.clone()))
    }

    fn put(&self, acc: Account) {
        self.lock().accounts.insert(acc.id.clone(), acc);
    }

    async fn test_and_list(
        &self,
        entry: &ProviderCatalogEntry,
        base_url: Option<&str>,
        secret: Option<&SecretString>,
    ) -> (ConnectionReport, Option<Vec<ModelInfo>>) {
        let req = ConnectionRequest {
            provider: entry,
            base_url,
            secret,
        };
        let report = redact(self.tester.test(req).await, secret);
        let models = if report.outcome.key_works() {
            self.lister.list_models(req).await.ok()
        } else {
            None
        };
        (report, models)
    }
}

#[async_trait]
impl AccountsHub for FakeAccountsHub {
    fn catalog(&self) -> Vec<ProviderCatalogEntry> {
        let ids: Vec<ProviderId> = self.lock().catalog.keys().cloned().collect();
        ids.iter().filter_map(|id| self.provider(id)).collect()
    }

    fn provider(&self, id: &ProviderId) -> Option<ProviderCatalogEntry> {
        let st = self.lock();
        let mut entry = st.catalog.get(id).cloned()?;
        for acc in st.accounts.values().filter(|a| &a.provider == id) {
            for m in &acc.models {
                if !entry.models.iter().any(|x| x.id == m.id) {
                    entry.models.push(m.clone());
                }
            }
        }
        Some(entry)
    }

    fn provider_state(&self, id: &ProviderId) -> AccountState {
        provider_state(self.lock().accounts.values().filter(|a| &a.provider == id))
    }

    fn accounts(&self) -> Vec<Account> {
        self.lock().accounts.values().cloned().collect()
    }

    fn account(&self, id: &AccountId) -> Option<Account> {
        self.lock().accounts.get(id).cloned()
    }

    fn set_pricing(&self, id: &ProviderId, pricing: PriceTable) -> Result<(), AccountsError> {
        let mut st = self.lock();
        let entry = st
            .catalog
            .get_mut(id)
            .ok_or_else(|| AccountsError::UnknownProvider(id.clone()))?;
        entry.pricing = pricing;
        Ok(())
    }

    async fn add_account(&self, new: NewAccount) -> Result<AccountId, AccountsError> {
        let entry = self.entry(&new.provider)?;
        if entry.compliance_status == ProviderApiStatus::Forbidden {
            return Err(AccountsError::ProviderForbidden(entry.id));
        }
        if entry.auth == AuthKind::OauthCli {
            return Err(invalid("ten dostawca wymaga logowania w oficjalnym CLI"));
        }
        if entry.auth == AuthKind::ApiKey && !new.secret.is_plausible_key() {
            return Err(invalid("klucz jest pusty albo zawiera niedozwolone znaki"));
        }
        match (&new.base_url, &entry.base_url) {
            (Some(url), _) => validate_base_url(url).map_err(AccountsError::InvalidInput)?,
            (None, None) => return Err(invalid("brak endpointu")),
            (None, Some(_)) => {}
        }
        let (id, now) = {
            let mut st = self.lock();
            st.next_id += 1;
            (AccountId::new(format!("acc-{}", st.next_id))?, st.now)
        };
        let secret = SecretName::for_account(&id);
        self.secrets.put(&secret, &new.secret)?;
        let label = match new.label.trim() {
            "" => entry.display_name.clone(),
            l => l.to_owned(),
        };
        self.put(Account {
            id: id.clone(),
            provider: new.provider,
            label,
            created_at: now,
            last_test: None,
            state: AccountState::Active,
            secret,
            base_url: new.base_url,
            assignments: new.assignments,
            cost_limit: new.cost_limit,
            source: new.source,
            models: Vec::new(),
        });
        Ok(id)
    }

    async fn test_account(&self, id: &AccountId) -> Result<TestSummary, AccountsError> {
        let mut acc = self.get(id)?;
        let entry = self.entry(&acc.provider)?;
        let secret = self.secrets.get(&acc.secret)?;
        let base = acc.base_url.clone().or_else(|| entry.base_url.clone());
        let (report, models) = self
            .test_and_list(&entry, base.as_deref(), secret.as_ref())
            .await;
        let summary = TestSummary {
            at: self.lock().now,
            outcome: report.outcome.clone(),
            latency_ms: report.latency_ms,
            models_found: models.as_ref().map(Vec::len),
        };
        if let Some(models) = models {
            acc.models = models;
        }
        acc.state = report.outcome.next_state(&acc.state);
        acc.last_test = Some(summary.clone());
        self.put(acc);
        Ok(summary)
    }

    async fn rotate(&self, id: &AccountId, secret: SecretString) -> Result<(), AccountsError> {
        let mut acc = self.get(id)?;
        if !secret.is_plausible_key() {
            return Err(invalid("klucz jest pusty albo zawiera niedozwolone znaki"));
        }
        self.secrets.put(&acc.secret, &secret)?;
        if acc.state != AccountState::Disabled {
            acc.state = AccountState::Active;
        }
        acc.last_test = None;
        self.put(acc);
        Ok(())
    }

    async fn remove(&self, id: &AccountId) -> Result<(), AccountsError> {
        let acc = self
            .lock()
            .accounts
            .remove(id)
            .ok_or_else(|| AccountsError::UnknownAccount(id.clone()))?;
        self.secrets.delete(&acc.secret)?;
        Ok(())
    }

    async fn set_disabled(&self, id: &AccountId, disabled: bool) -> Result<(), AccountsError> {
        let mut acc = self.get(id)?;
        acc.state = match (disabled, self.secrets.get(&acc.secret)?.is_some()) {
            (true, _) => AccountState::Disabled,
            (false, true) => AccountState::Active,
            (false, false) => AccountState::Unconfigured,
        };
        self.put(acc);
        Ok(())
    }

    async fn update_settings(
        &self,
        id: &AccountId,
        label: &str,
        assignments: Assignments,
        cost_limit: Option<CostLimit>,
    ) -> Result<(), AccountsError> {
        let mut acc = self.get(id)?;
        if !label.trim().is_empty() {
            label.trim().clone_into(&mut acc.label);
        }
        acc.assignments = assignments;
        acc.cost_limit = cost_limit;
        self.put(acc);
        Ok(())
    }

    async fn import_from_env(&self, env: &dyn EnvSource) -> Result<Vec<EnvImport>, AccountsError> {
        let mut out = Vec::new();
        for entry in self.catalog() {
            let allowed = entry.auth == AuthKind::ApiKey
                && entry.compliance_status != ProviderApiStatus::Forbidden;
            for var in entry.env_vars.iter().filter(|_| allowed) {
                let outcome = match env.var(var).filter(SecretString::is_plausible_key) {
                    None => EnvImportOutcome::NotSet,
                    Some(value) => self.import_one(&entry, var, value).await?,
                };
                out.push(EnvImport {
                    provider: entry.id.clone(),
                    var: var.clone(),
                    outcome,
                });
            }
        }
        Ok(out)
    }

    fn resolve_secret(&self, id: &AccountId, caller: &str) -> Result<SecretString, AccountsError> {
        if !caller.starts_with(SECRET_READER_PREFIX) {
            return Err(AccountsError::NotPermitted(caller.to_owned()));
        }
        let acc = self.get(id)?;
        if acc.state == AccountState::Disabled {
            return Err(AccountsError::AccountDisabled(id.clone()));
        }
        self.lock()
            .secret_reads
            .push((id.clone(), caller.to_owned()));
        self.secrets
            .get(&acc.secret)?
            .ok_or_else(|| AccountsError::SecretMissing(id.clone()))
    }

    async fn wizard_test(&self, wizard: &mut Wizard) -> Result<(), AccountsError> {
        let entry = wizard
            .provider()
            .cloned()
            .filter(|_| wizard.step() == WizardStep::TestConnection);
        let entry = entry.ok_or(WizardError::WrongStep {
            expected: WizardStep::TestConnection,
            actual: wizard.step(),
        })?;
        let secret = wizard.secret().cloned();
        let req = ConnectionRequest {
            provider: &entry,
            base_url: wizard.base_url(),
            secret: secret.as_ref(),
        };
        let report = redact(self.tester.test(req).await, secret.as_ref());
        Ok(wizard.record_test(report)?)
    }

    async fn wizard_discover(&self, wizard: &mut Wizard) -> Result<(), AccountsError> {
        let entry = wizard
            .provider()
            .cloned()
            .filter(|_| wizard.step() == WizardStep::DiscoverModels);
        let entry = entry.ok_or(WizardError::WrongStep {
            expected: WizardStep::DiscoverModels,
            actual: wizard.step(),
        })?;
        let secret = wizard.secret().cloned();
        let req = ConnectionRequest {
            provider: &entry,
            base_url: wizard.base_url(),
            secret: secret.as_ref(),
        };
        let result: Result<Vec<ModelInfo>, ModelListError> = self.lister.list_models(req).await;
        Ok(wizard.record_models(result)?)
    }

    async fn wizard_finish(&self, wizard: Wizard) -> Result<AccountId, AccountsError> {
        let outcome = wizard.finish()?;
        let id = self.add_account(outcome.account).await?;
        let mut acc = self.get(&id)?;
        acc.last_test = Some(TestSummary {
            at: self.lock().now,
            outcome: outcome.test.outcome.clone(),
            latency_ms: outcome.test.latency_ms,
            models_found: Some(outcome.models.len()),
        });
        acc.models = outcome.models;
        acc.state = outcome.test.outcome.next_state(&acc.state);
        self.put(acc);
        Ok(id)
    }
}
