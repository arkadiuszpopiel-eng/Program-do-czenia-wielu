//! Współdzielone testy kontraktowe (feature `contract-tests`), uruchamiane na `-impl` i `-fake`.
//!
//! Konwencja skryptu (hub pod testem musi mieć tester/lister, które jej przestrzegają —
//! `accounts-hub-fake::ScriptedConnectionTester::by_key_prefix` i `ScriptedModelLister::by_key_prefix`):
//! klucz zaczynający się od `sk-ok` → `Ok` + modele [`MODEL_A`], [`MODEL_B`];
//! `sk-bad` → `InvalidKey`; `sk-rate` → `RateLimited`; `sk-net` → `Network` z komunikatem
//! zawierającym klucz (sprawdza redakcję); brak klucza → `Ok` bez modeli (`Unsupported`).

mod flows;

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Mutex;

pub use flows::*;

use crate::{AccountsHub, EnvSource, ProviderCatalogEntry, SecretString};

/// Klucz poprawny.
pub const KEY_OK: &str = "sk-ok-contract-0001";
/// Drugi klucz poprawny (rotacja).
pub const KEY_OK_2: &str = "sk-ok-contract-0002";
/// Klucz odrzucany.
pub const KEY_INVALID: &str = "sk-bad-contract-0003";
/// Klucz z limitem zapytań.
pub const KEY_RATE: &str = "sk-rate-contract-0004";
/// Klucz, dla którego tester zwraca błąd sieci z kluczem w komunikacie.
pub const KEY_NET: &str = "sk-net-contract-0005";
/// Model wykrywany dla `sk-ok…`.
pub const MODEL_A: &str = "acme-large";
/// Drugi model wykrywany dla `sk-ok…`.
pub const MODEL_B: &str = "acme-small";

const ACME: &str = r#"
id = "acme"
display_name = "Acme AI"
kind = "chat"
auth = "api_key"
base_url = "https://api.acme.example/v1"
compat = "openai"
privacy_tag = "eu"
jurisdiction = "EU"
terms_url = "https://acme.example/terms"
compliance_status = "green"
notes = "fixture"
env_vars = ["ACME_API_KEY"]
[capabilities]
vision = true
tools = true
streaming = "unknown"
long_context = false
[pricing]
"#;

const BANNED: &str = r#"
id = "banned"
display_name = "Banned Plan"
kind = "chat"
auth = "api_key"
base_url = "https://api.banned.example"
compat = "openai"
privacy_tag = "cn-may-train"
jurisdiction = "CN"
terms_url = "TODO"
compliance_status = "forbidden"
notes = "fixture"
env_vars = ["BANNED_API_KEY"]
[capabilities]
vision = false
tools = false
streaming = false
long_context = false
[pricing]
"#;

const SELFHOST: &str = r#"
id = "selfhost"
display_name = "Własny endpoint"
kind = "chat"
auth = "api_key"
base_url = "TODO"
compat = "openai"
privacy_tag = "unknown"
jurisdiction = "unknown"
terms_url = "TODO"
compliance_status = "unverified"
notes = "fixture"
[capabilities]
vision = "unknown"
tools = "unknown"
streaming = "unknown"
long_context = "unknown"
[pricing]
"#;

const CLIONLY: &str = r#"
id = "clionly"
display_name = "Tylko CLI"
kind = "chat"
auth = "oauth_cli"
base_url = "TODO"
compat = "native"
privacy_tag = "unknown"
jurisdiction = "unknown"
terms_url = "TODO"
compliance_status = "gray"
notes = "fixture"
[capabilities]
vision = "unknown"
tools = "unknown"
streaming = "unknown"
long_context = "unknown"
[pricing]
"#;

/// Katalog-fixture: `acme` (zielony, `ACME_API_KEY`), `banned` (zabroniony),
/// `selfhost` (bez endpointu), `clionly` (logowanie w CLI).
pub fn fixture_catalog() -> Vec<ProviderCatalogEntry> {
    [
        (ACME, "acme"),
        (BANNED, "banned"),
        (SELFHOST, "selfhost"),
        (CLIONLY, "clionly"),
    ]
    .iter()
    .map(|(text, stem)| {
        ProviderCatalogEntry::from_toml(text, stem).unwrap_or_else(|e| panic!("fixture: {e}"))
    })
    .collect()
}

/// Środowisko-atrapa rejestrujące, o które zmienne pytano.
#[derive(Default)]
pub struct SpyEnv {
    values: Vec<(String, String)>,
    asked: Mutex<BTreeSet<String>>,
}

impl SpyEnv {
    /// Środowisko z parami (nazwa, wartość).
    pub fn new(values: &[(&str, &str)]) -> Self {
        Self {
            values: values
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            asked: Mutex::new(BTreeSet::new()),
        }
    }

    /// Nazwy zmiennych, o które zapytano.
    pub fn asked(&self) -> BTreeSet<String> {
        self.asked.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl EnvSource for SpyEnv {
    fn var(&self, name: &str) -> Option<SecretString> {
        if let Ok(mut asked) = self.asked.lock() {
            asked.insert(name.to_owned());
        }
        self.values
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| SecretString::from(v.as_str()))
            .filter(|s| !s.is_empty())
    }
}

/// Uruchamia cały zestaw; `factory(katalog)` daje świeży hub z testerem wg konwencji skryptu.
pub async fn run_all<H, F, Fut>(factory: F)
where
    H: AccountsHub,
    F: Fn(Vec<ProviderCatalogEntry>) -> Fut,
    Fut: Future<Output = H>,
{
    let fresh = || factory(fixture_catalog());
    unconfigured_without_accounts(&fresh().await).await;
    add_test_remove(&fresh().await).await;
    invalid_key_then_rotate(&fresh().await).await;
    network_error_is_redacted(&fresh().await).await;
    disable_and_enable(&fresh().await).await;
    rejects_forbidden_and_invalid_input(&fresh().await).await;
    wizard_happy_path(&fresh().await).await;
    wizard_bad_key_returns_to_key_step(&fresh().await).await;
    env_import_reads_only_catalog_vars(&fresh().await).await;
    pricing_and_models_in_catalog(&fresh().await).await;
    secrets_never_in_debug(&fresh().await).await;
}
