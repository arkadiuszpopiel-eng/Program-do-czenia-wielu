//! Atrapy portów: magazyn sekretów w pamięci, skryptowany tester połączenia i lister modeli,
//! środowisko z mapy.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use accounts_hub_contract::{
    ConnectionReport, ConnectionRequest, ConnectionTester, EnvSource, ModelId, ModelInfo,
    ModelListError, ModelLister, SecretName, SecretStore, SecretStoreError, SecretString,
    TestOutcome,
};
use async_trait::async_trait;

/// Modele zwracane dla kluczy `sk-ok…` (zgodne z `contract_tests::MODEL_A/MODEL_B`).
pub const SCRIPTED_MODELS: [&str; 2] = ["acme-large", "acme-small"];

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Magazyn sekretów w pamięci (zerowany przy zwolnieniu), z wstrzykiwaniem błędu.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    items: Mutex<BTreeMap<SecretName, SecretString>>,
    fail_next: Mutex<Option<SecretStoreError>>,
}

impl MemorySecretStore {
    /// Pusty magazyn.
    pub fn new() -> Self {
        Self::default()
    }

    /// Następna operacja zwróci ten błąd (jednorazowo).
    pub fn fail_next(&self, error: SecretStoreError) {
        *lock(&self.fail_next) = Some(error);
    }

    /// Liczba wpisów.
    pub fn len(&self) -> usize {
        lock(&self.items).len()
    }

    /// Czy pusty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn check(&self) -> Result<(), SecretStoreError> {
        lock(&self.fail_next).take().map_or(Ok(()), Err)
    }
}

impl SecretStore for MemorySecretStore {
    fn put(&self, name: &SecretName, value: &SecretString) -> Result<(), SecretStoreError> {
        self.check()?;
        lock(&self.items).insert(name.clone(), value.clone());
        Ok(())
    }

    fn get(&self, name: &SecretName) -> Result<Option<SecretString>, SecretStoreError> {
        self.check()?;
        Ok(lock(&self.items).get(name).cloned())
    }

    fn delete(&self, name: &SecretName) -> Result<bool, SecretStoreError> {
        self.check()?;
        Ok(lock(&self.items).remove(name).is_some())
    }

    fn list(&self) -> Result<Vec<SecretName>, SecretStoreError> {
        self.check()?;
        Ok(lock(&self.items).keys().cloned().collect())
    }
}

fn key_of(request: &ConnectionRequest<'_>) -> Option<String> {
    request.secret.map(|s| s.expose_secret().to_owned())
}

/// Tester połączenia: najpierw kolejka raportów (`push`), potem reguła prefiksu klucza:
/// `sk-ok` → Ok, `sk-bad` → InvalidKey, `sk-rate` → RateLimited, `sk-net` → Network
/// (komunikat celowo zawiera klucz — sprawdza redakcję w hubie), `sk-slow` → czeka 60 s,
/// brak klucza → Ok, inne → InvalidKey.
#[derive(Debug, Default)]
pub struct ScriptedConnectionTester {
    queue: Mutex<VecDeque<ConnectionReport>>,
    calls: Mutex<usize>,
}

impl ScriptedConnectionTester {
    /// Tester z regułą prefiksu klucza.
    pub fn by_key_prefix() -> Self {
        Self::default()
    }

    /// Następne wywołania zwrócą te raporty (FIFO).
    pub fn push(&self, report: ConnectionReport) {
        lock(&self.queue).push_back(report);
    }

    /// Liczba wywołań.
    pub fn calls(&self) -> usize {
        *lock(&self.calls)
    }
}

#[async_trait]
impl ConnectionTester for ScriptedConnectionTester {
    async fn test(&self, request: ConnectionRequest<'_>) -> ConnectionReport {
        *lock(&self.calls) += 1;
        if let Some(report) = lock(&self.queue).pop_front() {
            return report;
        }
        let outcome = match key_of(&request) {
            None => TestOutcome::Ok,
            Some(k) if k.starts_with("sk-ok") => TestOutcome::Ok,
            Some(k) if k.starts_with("sk-rate") => TestOutcome::RateLimited,
            Some(k) if k.starts_with("sk-net") => TestOutcome::Network {
                message: format!("connection reset (Authorization: Bearer {k})"),
            },
            Some(k) if k.starts_with("sk-slow") => {
                tokio::time::sleep(Duration::from_secs(60)).await;
                TestOutcome::Ok
            }
            Some(_) => TestOutcome::InvalidKey,
        };
        ConnectionReport {
            outcome,
            latency_ms: Some(12),
        }
    }
}

/// Lister modeli: kolejka wyników, potem reguła prefiksu (`sk-ok` → [`SCRIPTED_MODELS`],
/// `sk-bad` → InvalidKey, `sk-net` → Network z kluczem w komunikacie, inne → Unsupported).
#[derive(Debug, Default)]
pub struct ScriptedModelLister {
    queue: Mutex<VecDeque<Result<Vec<ModelInfo>, ModelListError>>>,
}

impl ScriptedModelLister {
    /// Lister z regułą prefiksu klucza.
    pub fn by_key_prefix() -> Self {
        Self::default()
    }

    /// Następne wywołania zwrócą te wyniki (FIFO).
    pub fn push(&self, result: Result<Vec<ModelInfo>, ModelListError>) {
        lock(&self.queue).push_back(result);
    }
}

/// Model o podanym identyfikatorze (bez metadanych).
pub fn model(id: &str) -> Option<ModelInfo> {
    Some(ModelInfo {
        id: ModelId::new(id).ok()?,
        display_name: None,
        capabilities: None,
        context_window: None,
    })
}

#[async_trait]
impl ModelLister for ScriptedModelLister {
    async fn list_models(
        &self,
        request: ConnectionRequest<'_>,
    ) -> Result<Vec<ModelInfo>, ModelListError> {
        if let Some(result) = lock(&self.queue).pop_front() {
            return result;
        }
        match key_of(&request) {
            Some(k) if k.starts_with("sk-ok") => {
                Ok(SCRIPTED_MODELS.iter().filter_map(|m| model(m)).collect())
            }
            Some(k) if k.starts_with("sk-bad") => Err(ModelListError::InvalidKey),
            Some(k) if k.starts_with("sk-net") => Err(ModelListError::Network {
                message: format!("reset for {k}"),
            }),
            _ => Err(ModelListError::Unsupported),
        }
    }
}

/// Środowisko z mapy (nazwa → wartość).
#[derive(Debug, Default, Clone)]
pub struct MapEnv(pub BTreeMap<String, String>);

impl MapEnv {
    /// Środowisko z par.
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        )
    }
}

impl EnvSource for MapEnv {
    fn var(&self, name: &str) -> Option<SecretString> {
        self.0
            .get(name)
            .map(|v| SecretString::from(v.as_str()))
            .filter(|s| !s.is_empty())
    }
}
