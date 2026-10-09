//! Kontrakt huba kont i kluczy (docs/modules/accounts-hub/SPEC.md, PLAN §5.6).
//!
//! Zawiera: model katalogu dostawców (`providers-catalog/*.toml`), konta i ich stany,
//! `SecretString` (zeroize, redakcja) i trait `SecretStore`, kreator jako maszynę stanów,
//! porty `ConnectionTester`/`ModelLister`/`EnvSource`/`CliProbe`, trait [`AccountsHub`]
//! i nazwy zdarzeń `accounts.*`. Wartość klucza nigdy nie trafia do typów serializowalnych.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod account;
mod api;
mod catalog;
mod ids;
mod model_use;
mod probe;
mod queue;
mod secret;
mod wizard;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use account::{
    Account, AccountErrorKind, AccountSource, AccountState, Assignments, ConnectionReport,
    CostLimit, NewAccount, ProbeDetail, TaskClass, TestOutcome, TestSummary, VoiceRole,
    provider_state,
};
pub use api::{
    AccountsError, AccountsHub, AccountsRepository, EVENT_KEY_ADDED, EVENT_KEY_REMOVED,
    EVENT_KEY_ROTATED, EVENT_KEY_TESTED, EVENT_STATE_CHANGED, EnvImport, EnvImportOutcome,
    SECRET_READER_PREFIX, event_kind,
};
pub use catalog::{
    AuthKind, CATALOG_PLACEHOLDER, Capabilities, CatalogError, Compat, ModelInfo, ModelPrice,
    PriceTable, ProviderCatalogEntry, ProviderKind, Tribool, is_env_var_name, validate_base_url,
};
pub use ids::{AccountId, ModelId, ProviderId, SecretName};
pub use model_use::{ModelUse, best_chat_model, chat_rank, is_snapshot, model_use};
pub use probe::{
    CliBridge, CliProbe, ConnectionRequest, ConnectionTester, EnvSource, KNOWN_CLI_BRIDGES,
    ModelListError, ModelLister, detect_cli_bridges_with, parse_version,
};
pub use queue::{
    MAX_QUEUE, ModelTier, QueueStrategy, fits_class, model_tier, suggest_queue, use_fits,
    validate_queue,
};
pub use secret::{REDACTED, SecretStore, SecretStoreError, SecretString};
pub use wizard::{Wizard, WizardError, WizardOutcome, WizardStep, WizardWarning};
