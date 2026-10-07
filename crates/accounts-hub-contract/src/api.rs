//! Trait `AccountsHub`, błędy, repozytorium metadanych i nazwy zdarzeń.

use async_trait::async_trait;
use core_bus_contract::EventKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::account::{Account, AccountState, Assignments, CostLimit, NewAccount, TestSummary};
use crate::catalog::{CatalogError, PriceTable, ProviderCatalogEntry};
use crate::ids::{AccountId, ProviderId};
use crate::probe::EnvSource;
use crate::secret::{SecretStoreError, SecretString};
use crate::wizard::{Wizard, WizardError};

/// Zdarzenie: dodano klucz (ładunek bez wartości klucza).
pub const EVENT_KEY_ADDED: &str = "accounts.key.added";
/// Zdarzenie: usunięto klucz.
pub const EVENT_KEY_REMOVED: &str = "accounts.key.removed";
/// Zdarzenie: przetestowano klucz.
pub const EVENT_KEY_TESTED: &str = "accounts.key.tested";
/// Zdarzenie: podmieniono klucz (rotacja).
pub const EVENT_KEY_ROTATED: &str = "accounts.key.rotated";
/// Zdarzenie: zmiana stanu konta (Router przelicza trasy bez restartu).
pub const EVENT_STATE_CHANGED: &str = "accounts.state_changed";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Prefiks modułów, które mogą odczytać wartość klucza (adaptery dostawców).
pub const SECRET_READER_PREFIX: &str = "providers-";

/// Błędy huba kont.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AccountsError {
    /// Nieznany dostawca.
    #[error("nieznany dostawca `{0}`")]
    UnknownProvider(ProviderId),
    /// Nieznane konto.
    #[error("nieznane konto `{0}`")]
    UnknownAccount(AccountId),
    /// Dostawca zabroniony przez rejestr/katalog zgodności.
    #[error("dostawca `{0}` jest zabroniony")]
    ProviderForbidden(ProviderId),
    /// Niepoprawne dane wejściowe.
    #[error("niepoprawne dane: {0}")]
    InvalidInput(String),
    /// Brak sekretu w magazynie.
    #[error("brak klucza konta `{0}` w magazynie sekretów")]
    SecretMissing(AccountId),
    /// Moduł nie może odczytać wartości klucza.
    #[error("moduł `{0}` nie może odczytać kluczy")]
    NotPermitted(String),
    /// Konto wyłączone przez użytkownika.
    #[error("konto `{0}` jest wyłączone")]
    AccountDisabled(AccountId),
    /// Błąd magazynu sekretów.
    #[error(transparent)]
    Secret(#[from] SecretStoreError),
    /// Błąd kreatora.
    #[error(transparent)]
    Wizard(#[from] WizardError),
    /// Błąd katalogu.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// Błąd zapisu metadanych.
    #[error("błąd zapisu metadanych kont: {0}")]
    Persist(String),
}

/// Wynik importu jednej zmiennej środowiskowej.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum EnvImportOutcome {
    /// Dodano nowe konto.
    Imported {
        /// Konto.
        account: AccountId,
    },
    /// Ten sam klucz jest już zapisany w koncie.
    AlreadyPresent {
        /// Konto.
        account: AccountId,
    },
    /// Zmienna nie jest ustawiona.
    NotSet,
}

/// Raport importu (tylko nazwy zmiennych, nigdy wartości).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvImport {
    /// Dostawca.
    pub provider: ProviderId,
    /// Nazwa zmiennej.
    pub var: String,
    /// Wynik.
    pub outcome: EnvImportOutcome,
}

/// Trwały zapis metadanych kont (bez sekretów) — produkcyjnie plik lub `core-config`.
pub trait AccountsRepository: Send + Sync {
    /// Wczytuje konta.
    fn load(&self) -> Result<Vec<Account>, AccountsError>;

    /// Zapisuje wszystkie konta (atomowo).
    fn save(&self, accounts: &[Account]) -> Result<(), AccountsError>;
}

/// Hub kont i kluczy (docs/modules/accounts-hub/SPEC.md).
#[async_trait]
pub trait AccountsHub: Send + Sync {
    /// Katalog dostawców (z modelami wykrytymi w kontach i cennikiem z konfiguracji).
    fn catalog(&self) -> Vec<ProviderCatalogEntry>;

    /// Jeden dostawca.
    fn provider(&self, id: &ProviderId) -> Option<ProviderCatalogEntry>;

    /// Stan dostawcy (brak kont = `Unconfigured`; Router go pomija).
    fn provider_state(&self, id: &ProviderId) -> AccountState;

    /// Wszystkie konta.
    fn accounts(&self) -> Vec<Account>;

    /// Jedno konto.
    fn account(&self, id: &AccountId) -> Option<Account>;

    /// Ustawia cennik dostawcy (z konfiguracji użytkownika).
    fn set_pricing(&self, id: &ProviderId, pricing: PriceTable) -> Result<(), AccountsError>;

    /// Dodaje konto (bez testu; stan `Active`). Klucz trafia wyłącznie do magazynu sekretów.
    async fn add_account(&self, new: NewAccount) -> Result<AccountId, AccountsError>;

    /// Test połączenia + wykrycie modeli; aktualizuje stan konta.
    async fn test_account(&self, id: &AccountId) -> Result<TestSummary, AccountsError>;

    /// Rotacja: nowy klucz w miejsce starego (bez restartu).
    async fn rotate(&self, id: &AccountId, secret: SecretString) -> Result<(), AccountsError>;

    /// Usuwa konto i jego klucz.
    async fn remove(&self, id: &AccountId) -> Result<(), AccountsError>;

    /// Wyłącza/włącza konto.
    async fn set_disabled(&self, id: &AccountId, disabled: bool) -> Result<(), AccountsError>;

    /// Zmienia etykietę, przypisania i limit kosztów.
    async fn update_settings(
        &self,
        id: &AccountId,
        label: &str,
        assignments: Assignments,
        cost_limit: Option<CostLimit>,
    ) -> Result<(), AccountsError>;

    /// Importuje klucze ze zmiennych wymienionych w katalogu (`env_vars`); tylko na życzenie.
    async fn import_from_env(&self, env: &dyn EnvSource) -> Result<Vec<EnvImport>, AccountsError>;

    /// Wartość klucza dla adaptera dostawcy (`caller` = id modułu `providers-*`).
    fn resolve_secret(&self, id: &AccountId, caller: &str) -> Result<SecretString, AccountsError>;

    /// Kreator: test połączenia dla bieżącego klucza (krok `TestConnection`).
    async fn wizard_test(&self, wizard: &mut Wizard) -> Result<(), AccountsError>;

    /// Kreator: wykrycie modeli (krok `DiscoverModels`).
    async fn wizard_discover(&self, wizard: &mut Wizard) -> Result<(), AccountsError>;

    /// Kreator: zapis konta (krok `Confirm`).
    async fn wizard_finish(&self, wizard: Wizard) -> Result<AccountId, AccountsError>;
}
