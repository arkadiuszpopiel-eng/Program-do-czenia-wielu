//! Adapter generyczny: budowa dostawcy z wpisu `providers-catalog/<id>.toml` + konta użytkownika
//! (PLAN §5.5 krok 0, §5.6). Nowy dostawca zgodny z OpenAI/Anthropic = nowy wpis, bez zmian w kodzie.

use std::collections::BTreeMap;
use std::sync::Arc;

use providers_contract::{
    ModelCapabilities, ModelKind, ModelProvider, PricingTable, ProviderPrivacy, SecretSource,
};
use serde::Deserialize;

use crate::anthropic::{ANTHROPIC_BASE_URL, AnthropicOptions, AnthropicProvider};
use crate::config::{AuthScheme, ConfigError, HttpConfig, ProviderProfile, RetryPolicy, Timeouts};
use crate::openai::{OPENAI_BASE_URL, OpenAiApi, OpenAiOptions, OpenAiProvider};

/// Wartość `true`/`false`/`"unknown"` z katalogu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Tribool {
    /// Znana wartość.
    Known(bool),
    /// `"unknown"` — traktowane ostrożnie jak `false`.
    Unknown(String),
}

impl Tribool {
    /// Czy na pewno `true`.
    pub fn is_true(&self) -> bool {
        matches!(self, Self::Known(true))
    }
}

/// Możliwości deklarowane w katalogu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogCapabilities {
    /// Wizja.
    pub vision: Tribool,
    /// Narzędzia.
    pub tools: Tribool,
    /// Strumieniowanie.
    pub streaming: Tribool,
    /// Długi kontekst.
    pub long_context: Tribool,
}

/// Rodzaj usług dostawcy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogKind {
    /// Czat.
    Chat,
    /// Mowa → tekst.
    Stt,
    /// Tekst → mowa.
    Tts,
    /// Wiele usług.
    Multi,
}

/// Uwierzytelnienie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogAuth {
    /// Klucz API (Credential Manager).
    ApiKey,
    /// Logowanie w oficjalnym CLI (most — nie ten moduł).
    OauthCli,
    /// Bez uwierzytelnienia.
    None,
}

/// Zgodność protokołu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogCompat {
    /// Endpoint zgodny z OpenAI (Chat Completions).
    Openai,
    /// Endpoint zgodny z Anthropic (Messages API).
    Anthropic,
    /// Adapter natywny (tylko `anthropic`, `openai`).
    Native,
}

/// Wpis katalogu (schemat: `providers-catalog/schema.json`). Parser jest tolerancyjny wobec
/// nowych pól (schemat waliduje CI); adapter czyta tylko pola, których potrzebuje.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CatalogEntry {
    /// Identyfikator = nazwa pliku.
    pub id: String,
    /// Nazwa w UI.
    pub display_name: String,
    /// Rodzaj usług.
    pub kind: CatalogKind,
    /// Uwierzytelnienie.
    pub auth: CatalogAuth,
    /// Endpoint albo znacznik braku wartości (wpis-szkic).
    pub base_url: String,
    /// Zgodność.
    pub compat: CatalogCompat,
    /// Możliwości.
    pub capabilities: CatalogCapabilities,
    /// Tag prywatności.
    pub privacy_tag: String,
    /// Jurysdykcja.
    pub jurisdiction: String,
    /// Cennik — w katalogu zawsze pusty (ceny w konfiguracji użytkownika).
    #[serde(default)]
    pub pricing: toml::Table,
    /// Regulamin.
    pub terms_url: String,
    /// Status zgodności: `green`/`gray`/`forbidden`/`unverified`.
    pub compliance_status: String,
    /// Uwagi.
    pub notes: String,
    /// Zmienne środowiskowe do importu klucza na życzenie użytkownika (kreator `accounts-hub`).
    #[serde(default)]
    pub env_vars: Vec<String>,
}

impl CatalogEntry {
    /// Parsuje wpis TOML.
    pub fn parse_toml(text: &str) -> Result<Self, ConfigError> {
        toml::from_str(text).map_err(|e| ConfigError::Catalog(e.to_string()))
    }

    /// Możliwości modelu wynikające z katalogu (ostrożnie: `unknown` = nie).
    pub fn default_capabilities(&self) -> ModelCapabilities {
        let vision = self.capabilities.vision.is_true();
        let mut kinds = vec![ModelKind::Chat];
        if vision {
            kinds.push(ModelKind::Vision);
        }
        ModelCapabilities {
            kinds,
            tools: self.capabilities.tools.is_true(),
            vision,
            ..ModelCapabilities::default()
        }
    }
}

/// Konto użytkownika dla wpisu katalogu (kreator „Dodaj dostawcę / konto / klucz").
#[derive(Clone)]
pub struct AccountProfile {
    /// Źródło klucza (Credential Manager przez `accounts-hub`).
    pub key: Arc<dyn SecretSource>,
    /// Nadpisanie endpointu (wymagane, gdy katalog nie podaje adresu).
    pub base_url: Option<String>,
    /// Model domyślny.
    pub default_model: Option<String>,
    /// Możliwości modeli (nadpisują katalog).
    pub models: BTreeMap<String, ModelCapabilities>,
    /// Cennik (konfiguracja użytkownika).
    pub pricing: PricingTable,
    /// Limity czasu.
    pub timeouts: Option<Timeouts>,
    /// Ponawianie.
    pub retry: Option<RetryPolicy>,
    /// Format API dla natywnego OpenAI (domyślnie Responses).
    pub openai_api: Option<OpenAiApi>,
}

impl AccountProfile {
    /// Konto z samym kluczem.
    pub fn new(key: Arc<dyn SecretSource>) -> Self {
        Self {
            key,
            base_url: None,
            default_model: None,
            models: BTreeMap::new(),
            pricing: PricingTable::new(),
            timeouts: None,
            retry: None,
            openai_api: None,
        }
    }
}

/// Buduje dostawcę z wpisu katalogu i konta.
pub fn build_provider(
    entry: &CatalogEntry,
    account: AccountProfile,
) -> Result<Arc<dyn ModelProvider>, ConfigError> {
    let unsupported = |why: &str| ConfigError::Unsupported(entry.id.clone(), why.to_owned());
    if entry.compliance_status == "forbidden" {
        return Err(ConfigError::Forbidden(entry.id.clone()));
    }
    if matches!(entry.kind, CatalogKind::Stt | CatalogKind::Tts) {
        return Err(unsupported("usługa głosowa — obsługują ją moduły voice-*"));
    }
    if entry.auth == CatalogAuth::OauthCli {
        return Err(unsupported(
            "logowanie CLI — to most (AgentBackend), nie API",
        ));
    }
    if entry.capabilities.streaming == Tribool::Known(false) {
        return Err(unsupported("brak strumieniowania"));
    }
    let native_default = match (entry.compat, entry.id.as_str()) {
        (CatalogCompat::Native, "anthropic") => Some(ANTHROPIC_BASE_URL),
        (CatalogCompat::Native, "openai") => Some(OPENAI_BASE_URL),
        (CatalogCompat::Native, _) => return Err(unsupported("brak adaptera natywnego")),
        _ => None,
    };
    // Wpis-szkic ma znacznik zamiast adresu — za adres uznajemy tylko URL http(s).
    let catalog_url = ["https://", "http://"]
        .iter()
        .any(|p| entry.base_url.starts_with(p))
        .then(|| entry.base_url.clone());
    let base_url = account
        .base_url
        .clone()
        .or(catalog_url)
        .or(native_default.map(str::to_owned))
        .ok_or_else(|| ConfigError::MissingBaseUrl(entry.id.clone()))?;

    let mut profile = ProviderProfile::new(entry.id.as_str());
    profile.privacy = ProviderPrivacy::new(&entry.privacy_tag, &entry.jurisdiction);
    profile.default_model.clone_from(&account.default_model);
    profile.pricing = account.pricing.clone();
    profile.models = account.models.clone();
    if let Some(model) = &account.default_model
        && entry.compat != CatalogCompat::Native
    {
        profile
            .models
            .entry(model.clone())
            .or_insert_with(|| entry.default_capabilities());
    }
    let anthropic_like = matches!(entry.compat, CatalogCompat::Anthropic)
        || native_default == Some(ANTHROPIC_BASE_URL);
    let auth = match (entry.auth, anthropic_like) {
        (CatalogAuth::None, _) => AuthScheme::None,
        (_, true) => AuthScheme::XApiKey,
        (_, false) => AuthScheme::Bearer,
    };
    let mut http = HttpConfig::new(base_url, auth);
    http.timeouts = account.timeouts.unwrap_or_default();
    http.retry = account.retry.unwrap_or_default();

    let provider: Arc<dyn ModelProvider> = match (entry.compat, anthropic_like) {
        (CatalogCompat::Native, true) => Arc::new(AnthropicProvider::new(
            profile,
            http,
            account.key,
            AnthropicOptions::native(),
        )?),
        (CatalogCompat::Native, false) => {
            let options = match account.openai_api.unwrap_or(OpenAiApi::Responses) {
                OpenAiApi::Responses => OpenAiOptions::native(),
                OpenAiApi::ChatCompletions => OpenAiOptions::native_chat(),
            };
            Arc::new(OpenAiProvider::new(profile, http, account.key, options)?)
        }
        (CatalogCompat::Anthropic, _) => Arc::new(AnthropicProvider::new(
            profile,
            http,
            account.key,
            AnthropicOptions::compatible(),
        )?),
        (CatalogCompat::Openai, _) => Arc::new(OpenAiProvider::new(
            profile,
            http,
            account.key,
            OpenAiOptions::compatible(),
        )?),
    };
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XAI: &str = r#"
id = "xai"
display_name = "xAI"
kind = "chat"
auth = "api_key"
base_url = "https://api.x.ai/v1"
compat = "openai"
privacy_tag = "xai-retention-30d"
jurisdiction = "unknown"
terms_url = "https://x.ai/legal"
compliance_status = "unverified"
notes = ""
[capabilities]
vision = true
tools = "unknown"
streaming = true
long_context = "unknown"
[pricing]
"#;

    #[test]
    fn parses_tribools_and_tolerates_new_fields() {
        let e = CatalogEntry::parse_toml(XAI).unwrap();
        assert!(e.capabilities.vision.is_true());
        assert!(!e.capabilities.tools.is_true());
        let caps = e.default_capabilities();
        assert!(caps.vision && !caps.tools);
        assert!(CatalogEntry::parse_toml(&format!("future_field = 1\n{XAI}")).is_ok());
        assert!(e.env_vars.is_empty());
        assert!(matches!(
            CatalogEntry::parse_toml("id = 1"),
            Err(ConfigError::Catalog(_))
        ));
    }
}
