//! Katalog dostawców (`providers-catalog/*.toml`, schemat `providers-catalog/schema.json`).

use std::collections::BTreeMap;

use compliance_contract::{
    Jurisdiction, PrivacyTag, ProviderApiStatus, ProviderPolicyInput, RouteTags,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{ModelId, ProviderId};

/// Wartość „tak / nie / nie wiadomo” (w TOML: `true`, `false`, `"unknown"`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Tribool {
    /// Tak.
    Yes,
    /// Nie.
    No,
    /// Nie wiadomo (Router traktuje jak brak).
    #[default]
    Unknown,
}

impl Tribool {
    /// Tylko potwierdzone „tak”.
    pub fn is_yes(self) -> bool {
        self == Tribool::Yes
    }
}

impl Serialize for Tribool {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Tribool::Yes => s.serialize_bool(true),
            Tribool::No => s.serialize_bool(false),
            Tribool::Unknown => s.serialize_str("unknown"),
        }
    }
}

impl<'de> Deserialize<'de> for Tribool {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Bool(bool),
            Text(String),
        }
        match Raw::deserialize(d)? {
            Raw::Bool(true) => Ok(Tribool::Yes),
            Raw::Bool(false) => Ok(Tribool::No),
            Raw::Text(t) if t == "unknown" => Ok(Tribool::Unknown),
            Raw::Text(t) => Err(serde::de::Error::custom(format!(
                "oczekiwano true/false/\"unknown\", jest `{t}`"
            ))),
        }
    }
}

impl JsonSchema for Tribool {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Tribool".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "anyOf": [ { "type": "boolean" }, { "const": "unknown" } ] })
    }
}

/// Możliwości dostawcy lub modelu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// Wizja (obrazy na wejściu).
    pub vision: Tribool,
    /// Narzędzia (function calling).
    pub tools: Tribool,
    /// Strumieniowanie.
    pub streaming: Tribool,
    /// Długi kontekst.
    pub long_context: Tribool,
}

/// Rodzaj usług dostawcy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Czat.
    Chat,
    /// Rozpoznawanie mowy.
    Stt,
    /// Synteza mowy.
    Tts,
    /// Wiele rodzajów.
    Multi,
}

/// Sposób uwierzytelnienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// Klucz API (w Credential Manager).
    ApiKey,
    /// Logowanie użytkownika w oficjalnym CLI (most, F4) — Alfa nie widzi tokenów.
    OauthCli,
    /// Bez uwierzytelnienia (np. lokalny endpoint).
    None,
}

/// Adapter protokołu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Compat {
    /// Generyczny zgodny z OpenAI.
    Openai,
    /// Generyczny zgodny z Anthropic.
    Anthropic,
    /// Natywny adapter.
    Native,
}

/// Cena modelu w mikro-USD za 1 mln tokenów (liczby całkowite; 3 USD/Mtok = 3 000 000).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelPrice {
    /// Tokeny wejściowe.
    pub input_micro_usd_per_mtok: u64,
    /// Tokeny wyjściowe.
    pub output_micro_usd_per_mtok: u64,
    /// Odczyt z cache promptów.
    #[serde(default)]
    pub cache_read_micro_usd_per_mtok: u64,
    /// Zapis do cache promptów.
    #[serde(default)]
    pub cache_write_micro_usd_per_mtok: u64,
}

/// Cennik dostawcy: model → cena; klucz `*` = cena domyślna. Pochodzi z konfiguracji
/// użytkownika, nigdy z kodu ani z katalogu (PLAN §5.5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct PriceTable(pub BTreeMap<ModelId, ModelPrice>);

impl PriceTable {
    /// Cena modelu: dokładne dopasowanie, potem `*`; `None` = koszt nieznany (nie 0).
    pub fn price_for(&self, model: &ModelId) -> Option<ModelPrice> {
        self.0
            .get(model)
            .or_else(|| {
                self.0
                    .iter()
                    .find(|(k, _)| k.as_str() == "*")
                    .map(|(_, v)| v)
            })
            .copied()
    }

    /// Czy pusty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Model wykryty u dostawcy (np. przez Models API).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelInfo {
    /// Identyfikator modelu.
    pub id: ModelId,
    /// Nazwa wyświetlana, jeśli dostawca ją podaje.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Możliwości modelu, jeśli znane.
    #[serde(default)]
    pub capabilities: Option<Capabilities>,
    /// Okno kontekstu w tokenach, jeśli znane.
    #[serde(default)]
    pub context_window: Option<u32>,
}

/// Wpis katalogu w postaci pliku (dokładnie pola ze `schema.json`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogFile {
    id: ProviderId,
    display_name: String,
    kind: ProviderKind,
    auth: AuthKind,
    base_url: String,
    compat: Compat,
    capabilities: Capabilities,
    privacy_tag: PrivacyTag,
    jurisdiction: Jurisdiction,
    pricing: BTreeMap<String, toml::Value>,
    terms_url: String,
    compliance_status: ProviderApiStatus,
    notes: String,
    #[serde(default)]
    env_vars: Vec<String>,
}

/// Dostawca: wpis katalogu + modele wykryte i cennik z konfiguracji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderCatalogEntry {
    /// Identyfikator (= nazwa pliku).
    pub id: ProviderId,
    /// Nazwa w UI.
    pub display_name: String,
    /// Rodzaj usług.
    pub kind: ProviderKind,
    /// Uwierzytelnienie.
    pub auth: AuthKind,
    /// Endpoint; `None` = nieznany w katalogu ([`CATALOG_PLACEHOLDER`]), podaje go użytkownik.
    pub base_url: Option<String>,
    /// Adapter.
    pub compat: Compat,
    /// Możliwości.
    pub capabilities: Capabilities,
    /// Tag prywatności.
    pub privacy_tag: PrivacyTag,
    /// Jurysdykcja.
    pub jurisdiction: Jurisdiction,
    /// Link do regulaminu; `None` = [`CATALOG_PLACEHOLDER`].
    pub terms_url: Option<String>,
    /// Status zgodności API.
    pub compliance_status: ProviderApiStatus,
    /// Uwagi.
    pub notes: String,
    /// Zmienne środowiskowe, z których wolno zaimportować klucz (na życzenie użytkownika).
    pub env_vars: Vec<String>,
    /// Modele wykryte po teście konta (w katalogu zawsze puste).
    pub models: Vec<ModelInfo>,
    /// Cennik z konfiguracji użytkownika (w katalogu zawsze pusty).
    pub pricing: PriceTable,
}

/// Błędy katalogu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// Błąd składni TOML lub niezgodność z modelem.
    #[error("{file}: błąd składni wpisu katalogu: {reason}")]
    Syntax {
        /// Plik.
        file: String,
        /// Powód.
        reason: String,
    },
    /// Niezgodność z JSON Schema katalogu.
    #[error("{file}: niezgodność ze schematem: {reason}")]
    Schema {
        /// Plik.
        file: String,
        /// Powód.
        reason: String,
    },
    /// Reguła semantyczna.
    #[error("{file}: {reason}")]
    Invalid {
        /// Plik.
        file: String,
        /// Powód.
        reason: String,
    },
    /// Błąd odczytu katalogu.
    #[error("{file}: błąd odczytu: {reason}")]
    Io {
        /// Plik lub katalog.
        file: String,
        /// Powód.
        reason: String,
    },
}

/// Znacznik „wartość nieznana — uzupełnia użytkownik” w polach `base_url`/`terms_url` katalogu
/// (format danych `providers-catalog`, nie komentarz w kodzie).
pub const CATALOG_PLACEHOLDER: &str = "TODO";

/// Czy nazwa zmiennej środowiskowej ma bezpieczny format (`[A-Z][A-Z0-9_]*`).
pub fn is_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Endpoint podany przez użytkownika: `https://…`, a `http://` tylko dla hosta lokalnego.
pub fn validate_base_url(url: &str) -> Result<(), String> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("https://") && lower.len() > "https://".len() {
        return Ok(());
    }
    let local = ["http://localhost", "http://127.0.0.1", "http://[::1]"];
    if local.iter().any(|p| {
        lower
            .strip_prefix(p)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([':', '/']))
    }) {
        return Ok(());
    }
    Err(format!(
        "endpoint `{url}` musi zaczynać się od https:// (http:// tylko dla localhost)"
    ))
}

impl ProviderCatalogEntry {
    /// Parsuje wpis z TOML i sprawdza reguły semantyczne (`id` = nazwa pliku, pusty cennik,
    /// adresy `https://` albo znacznik, nazwy zmiennych środowiskowych). Walidację JSON Schema
    /// wykonuje dodatkowo `accounts-hub-impl`.
    pub fn from_toml(text: &str, file_stem: &str) -> Result<Self, CatalogError> {
        let file = format!("{file_stem}.toml");
        let raw: CatalogFile = toml::from_str(text).map_err(|e| CatalogError::Syntax {
            file: file.clone(),
            reason: e.message().to_owned(),
        })?;
        let invalid = |reason: String| CatalogError::Invalid {
            file: file.clone(),
            reason,
        };
        if raw.id.as_str() != file_stem {
            return Err(invalid(format!("id `{}` różni się od nazwy pliku", raw.id)));
        }
        if !raw.pricing.is_empty() {
            return Err(invalid(
                "cennik w katalogu musi być pusty (tylko konfiguracja)".into(),
            ));
        }
        let url_field = |value: &str, field: &str| -> Result<Option<String>, CatalogError> {
            if value == CATALOG_PLACEHOLDER {
                return Ok(None);
            }
            if !value.starts_with("https://") || value.len() <= "https://".len() {
                return Err(invalid(format!(
                    "{field} musi być https:// albo {CATALOG_PLACEHOLDER}"
                )));
            }
            Ok(Some(value.to_owned()))
        };
        let base_url = url_field(&raw.base_url, "base_url")?;
        let terms_url = url_field(&raw.terms_url, "terms_url")?;
        if let Some(bad) = raw.env_vars.iter().find(|v| !is_env_var_name(v)) {
            return Err(invalid(format!("niepoprawna nazwa zmiennej `{bad}`")));
        }
        if raw.display_name.trim().is_empty() {
            return Err(invalid("pusta nazwa wyświetlana".into()));
        }
        Ok(Self {
            id: raw.id,
            display_name: raw.display_name,
            kind: raw.kind,
            auth: raw.auth,
            base_url,
            compat: raw.compat,
            capabilities: raw.capabilities,
            privacy_tag: raw.privacy_tag,
            jurisdiction: raw.jurisdiction,
            terms_url,
            compliance_status: raw.compliance_status,
            notes: raw.notes,
            env_vars: raw.env_vars,
            models: Vec::new(),
            pricing: PriceTable::default(),
        })
    }

    /// Dane dla modułu zgodności (tagi + status API).
    pub fn policy_input(&self) -> ProviderPolicyInput {
        ProviderPolicyInput {
            provider: self.id.as_str().to_owned(),
            tags: RouteTags {
                privacy: [self.privacy_tag].into(),
                jurisdiction: self.jurisdiction.clone(),
            },
            api_status: self.compliance_status,
        }
    }
}
