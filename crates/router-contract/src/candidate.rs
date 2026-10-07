//! Kandydat trasy `{dostawca, model}` i rodzaj trasy (lokalna / API).

use std::fmt;
use std::str::FromStr;

use providers_contract::ProviderId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Kandydat trasy. Tekstowo `dostawca:model` (np. `anthropic:claude-opus-5-5`, `local:bielik-4.5b-q8_0`)
/// — ten sam zapis w konfiguracji (`prefer = [...]`), w `ChatRequest::model` (przypięcie) i w
/// `ProviderEvent::Started::model` zwracanym przez Router (pochodzenie bloków myślenia).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Candidate {
    /// Dostawca (wpis katalogu albo `local`).
    pub provider: ProviderId,
    /// Model u dostawcy.
    pub model: String,
}

impl Candidate {
    /// Nowy kandydat.
    pub fn new(provider: impl Into<ProviderId>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }

    /// Zapis `dostawca:model`.
    pub fn qualified(&self) -> String {
        format!("{}:{}", self.provider, self.model)
    }

    /// Parsuje `dostawca:model` (dostawca bez `:`; model niepusty, może zawierać `:` i `/`).
    pub fn parse(text: &str) -> Option<Self> {
        let (provider, model) = text.split_once(':')?;
        let ok_provider = !provider.is_empty()
            && provider
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        (ok_provider && !model.trim().is_empty() && !model.chars().any(char::is_whitespace))
            .then(|| Self::new(provider, model))
    }
}

impl fmt::Display for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.model)
    }
}

impl FromStr for Candidate {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
            .ok_or_else(|| format!("niepoprawny kandydat `{s}` (oczekiwano `dostawca:model`)"))
    }
}

impl Serialize for Candidate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.qualified())
    }
}

impl<'de> Deserialize<'de> for Candidate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Candidate {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Candidate".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^[a-z0-9_-]+:\\S+$",
            "description": "Kandydat trasy `dostawca:model`."
        })
    }
}

/// Rodzaj trasy dostawcy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    /// Lokalny model (llama.cpp) — bez ruchu sieciowego: rejestr zgodności i jurysdykcja
    /// nie dotyczą, zawsze dozwolony dla sesji „prywatne" (ADR 0014: `local_only`).
    Local,
    /// API chmurowe — trasa `<dostawca>.api` w rejestrze zgodności.
    Api,
}
