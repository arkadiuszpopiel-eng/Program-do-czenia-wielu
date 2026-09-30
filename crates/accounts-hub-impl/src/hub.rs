//! `AccountsHubService`: stan, budowa, pomocnicze operacje (zapis metadanych, zdarzenia, testy).

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

use accounts_hub_contract::{
    Account, AccountId, AccountState, AccountsError, AccountsRepository, AuthKind,
    ConnectionReport, ConnectionRequest, ConnectionTester, EVENT_STATE_CHANGED, ModelInfo,
    ModelListError, ModelLister, ProviderCatalogEntry, ProviderId, SecretStore, SecretString,
    TestOutcome, event_kind,
};
use chrono::{DateTime, Utc};
use compliance_contract::{Compliance, RouteId, RouteStatus};
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{ManifestError, ModuleManifest};

use crate::MODULE_TOML;

/// Domyślny limit czasu testu połączenia (SPEC: ≤ 10 s).
pub const DEFAULT_TEST_TIMEOUT: Duration = Duration::from_secs(10);

type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;
type IdGen = Arc<dyn Fn() -> String + Send + Sync>;

/// Budowniczy huba.
pub struct HubBuilder {
    catalog: Vec<ProviderCatalogEntry>,
    secrets: Arc<dyn SecretStore>,
    tester: Arc<dyn ConnectionTester>,
    lister: Arc<dyn ModelLister>,
    repo: Option<Arc<dyn AccountsRepository>>,
    compliance: Option<Arc<dyn Compliance>>,
    clock: Clock,
    ids: IdGen,
    timeout: Duration,
}

impl HubBuilder {
    /// Trwały zapis metadanych kont.
    #[must_use]
    pub fn repository(mut self, repo: Arc<dyn AccountsRepository>) -> Self {
        self.repo = Some(repo);
        self
    }

    /// Moduł zgodności (trasa `<provider>.api` zabroniona → dodanie konta odrzucone).
    #[must_use]
    pub fn compliance(mut self, compliance: Arc<dyn Compliance>) -> Self {
        self.compliance = Some(compliance);
        self
    }

    /// Zegar (testy deterministyczne).
    #[must_use]
    pub fn clock(mut self, clock: impl Fn() -> DateTime<Utc> + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// Generator sufiksów identyfikatorów kont (`acc-<sufiks>`).
    #[must_use]
    pub fn id_suffixes(mut self, ids: impl Fn() -> String + Send + Sync + 'static) -> Self {
        self.ids = Arc::new(ids);
        self
    }

    /// Limit czasu testu połączenia i wykrywania modeli.
    #[must_use]
    pub fn test_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Buduje hub; wczytuje metadane kont i oznacza konta bez klucza jako „nieskonfigurowane”.
    pub fn build(self) -> Result<AccountsHubService, AccountsError> {
        let manifest = ModuleManifest::parse_toml(MODULE_TOML)
            .map_err(|e: ManifestError| AccountsError::Persist(e.to_string()))?;
        let mut accounts = BTreeMap::new();
        if let Some(repo) = &self.repo {
            let stored = self.secrets.list()?;
            for mut acc in repo.load()? {
                let needs_secret = self
                    .catalog
                    .iter()
                    .find(|p| p.id == acc.provider)
                    .is_none_or(|p| p.auth == AuthKind::ApiKey);
                if needs_secret && !stored.contains(&acc.secret) {
                    acc.state = AccountState::Unconfigured;
                }
                accounts.insert(acc.id.clone(), acc);
            }
        }
        Ok(AccountsHubService {
            manifest,
            catalog: RwLock::new(
                self.catalog
                    .into_iter()
                    .map(|p| (p.id.clone(), p))
                    .collect(),
            ),
            accounts: RwLock::new(accounts),
            secrets: self.secrets,
            tester: self.tester,
            lister: self.lister,
            repo: self.repo,
            compliance: self.compliance,
            bus: RwLock::new(None),
            clock: self.clock,
            ids: self.ids,
            timeout: self.timeout,
        })
    }
}

/// Hub kont i kluczy.
pub struct AccountsHubService {
    pub(crate) manifest: ModuleManifest,
    pub(crate) catalog: RwLock<BTreeMap<ProviderId, ProviderCatalogEntry>>,
    pub(crate) accounts: RwLock<BTreeMap<AccountId, Account>>,
    pub(crate) secrets: Arc<dyn SecretStore>,
    tester: Arc<dyn ConnectionTester>,
    lister: Arc<dyn ModelLister>,
    repo: Option<Arc<dyn AccountsRepository>>,
    compliance: Option<Arc<dyn Compliance>>,
    pub(crate) bus: RwLock<Option<Arc<dyn EventBus>>>,
    clock: Clock,
    ids: IdGen,
    timeout: Duration,
}

fn guard<T>(r: Result<T, std::sync::PoisonError<T>>) -> T {
    r.unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl AccountsHubService {
    /// Budowniczy z wymaganymi portami.
    pub fn builder(
        catalog: Vec<ProviderCatalogEntry>,
        secrets: Arc<dyn SecretStore>,
        tester: Arc<dyn ConnectionTester>,
        lister: Arc<dyn ModelLister>,
    ) -> HubBuilder {
        HubBuilder {
            catalog,
            secrets,
            tester,
            lister,
            repo: None,
            compliance: None,
            clock: Arc::new(Utc::now),
            ids: Arc::new(|| uuid::Uuid::new_v4().simple().to_string()),
            timeout: DEFAULT_TEST_TIMEOUT,
        }
    }

    pub(crate) fn catalog_read(
        &self,
    ) -> RwLockReadGuard<'_, BTreeMap<ProviderId, ProviderCatalogEntry>> {
        guard(self.catalog.read())
    }

    pub(crate) fn catalog_write(
        &self,
    ) -> RwLockWriteGuard<'_, BTreeMap<ProviderId, ProviderCatalogEntry>> {
        guard(self.catalog.write())
    }

    pub(crate) fn accounts_read(&self) -> RwLockReadGuard<'_, BTreeMap<AccountId, Account>> {
        guard(self.accounts.read())
    }

    pub(crate) fn accounts_write(&self) -> RwLockWriteGuard<'_, BTreeMap<AccountId, Account>> {
        guard(self.accounts.write())
    }

    pub(crate) fn now(&self) -> DateTime<Utc> {
        (self.clock)()
    }

    pub(crate) fn new_id(&self) -> Result<AccountId, AccountsError> {
        AccountId::new(format!("acc-{}", (self.ids)()))
    }

    pub(crate) fn provider_entry(
        &self,
        id: &ProviderId,
    ) -> Result<ProviderCatalogEntry, AccountsError> {
        self.catalog_read()
            .get(id)
            .cloned()
            .ok_or_else(|| AccountsError::UnknownProvider(id.clone()))
    }

    /// Zabroniony w katalogu albo w module zgodności (trasa `<provider>.api`).
    pub(crate) fn ensure_allowed(&self, entry: &ProviderCatalogEntry) -> Result<(), AccountsError> {
        let forbidden_catalog =
            entry.compliance_status == compliance_contract::ProviderApiStatus::Forbidden;
        let forbidden_registry = self.compliance.as_ref().is_some_and(|c| {
            RouteId::api(entry.id.as_str())
                .and_then(|r| c.effective_status(&r))
                .is_some_and(|s| s.status == RouteStatus::Forbidden)
        });
        if forbidden_catalog || forbidden_registry {
            return Err(AccountsError::ProviderForbidden(entry.id.clone()));
        }
        Ok(())
    }

    /// Zapisuje metadane (jeśli jest repozytorium).
    pub(crate) fn persist(&self) -> Result<(), AccountsError> {
        if let Some(repo) = &self.repo {
            let snapshot: Vec<Account> = self.accounts_read().values().cloned().collect();
            repo.save(&snapshot)?;
        }
        Ok(())
    }

    pub(crate) async fn publish(&self, name: &str, level: Level, payload: serde_json::Value) {
        let bus = guard(self.bus.read()).clone();
        if let Some(bus) = bus {
            // Zdarzenia są informacyjne; błąd magistrali nie cofa operacji na kontach.
            let _ = bus
                .publish(Event::new(event_kind(name), level, payload))
                .await;
        }
    }

    pub(crate) async fn publish_state(&self, acc: &Account, from: &AccountState) {
        if *from != acc.state {
            let payload = serde_json::json!({
                "account": acc.id, "provider": acc.provider, "from": from, "to": acc.state,
            });
            self.publish(EVENT_STATE_CHANGED, Level::Info, payload)
                .await;
        }
    }

    /// Test połączenia z limitem czasu; komunikaty redagowane (klucz nie może wyciec).
    pub(crate) async fn run_test(
        &self,
        entry: &ProviderCatalogEntry,
        base_url: Option<&str>,
        secret: Option<&SecretString>,
    ) -> ConnectionReport {
        let request = ConnectionRequest {
            provider: entry,
            base_url,
            secret,
        };
        let started = Instant::now();
        let report = match tokio::time::timeout(self.timeout, self.tester.test(request)).await {
            Ok(report) => report,
            Err(_) => ConnectionReport {
                outcome: TestOutcome::Timeout,
                latency_ms: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
            },
        };
        redact_report(report, secret)
    }

    /// Wykrywanie modeli z limitem czasu i redakcją.
    pub(crate) async fn run_listing(
        &self,
        entry: &ProviderCatalogEntry,
        base_url: Option<&str>,
        secret: Option<&SecretString>,
    ) -> Result<Vec<ModelInfo>, ModelListError> {
        let request = ConnectionRequest {
            provider: entry,
            base_url,
            secret,
        };
        match tokio::time::timeout(self.timeout, self.lister.list_models(request)).await {
            Ok(Err(ModelListError::Network { message })) => Err(ModelListError::Network {
                message: redact(&message, secret),
            }),
            Ok(other) => other,
            Err(_) => Err(ModelListError::Timeout),
        }
    }
}

fn redact(text: &str, secret: Option<&SecretString>) -> String {
    secret.map_or_else(|| text.to_owned(), |s| s.redact_in(text))
}

fn redact_report(mut report: ConnectionReport, secret: Option<&SecretString>) -> ConnectionReport {
    report.outcome = match report.outcome {
        TestOutcome::Network { message } => TestOutcome::Network {
            message: redact(&message, secret),
        },
        TestOutcome::Unsupported { message } => TestOutcome::Unsupported {
            message: redact(&message, secret),
        },
        other => other,
    };
    report
}
