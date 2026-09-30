//! Atrapa huba kont (docs/modules/accounts-hub/SPEC.md „Fake”): katalog z fixture'ów,
//! sekrety w pamięci, skryptowane raporty testu połączenia i listy modeli.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod hub;
mod import;
mod ports;

pub use hub::FakeAccountsHub;
pub use ports::{
    MapEnv, MemorySecretStore, SCRIPTED_MODELS, ScriptedConnectionTester, ScriptedModelLister,
    model,
};
