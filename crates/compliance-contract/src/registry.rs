//! Model pliku `docs/compliance/compliance-registry.json` (format wersjonowany) i jego walidacja.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::status::{RouteId, RouteStatus, is_kebab};
use crate::tags::{Jurisdiction, PrivacyTag};

/// Obsługiwane wersje formatu rejestru.
pub const SUPPORTED_SCHEMA_VERSIONS: [u32; 1] = [1];

/// Tryb trasy (`mode` w rejestrze).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum RouteMode {
    /// Oficjalny tryb nieinteraktywny CLI (`claude -p` itp.).
    #[serde(rename = "cli-p")]
    CliHeadless,
    /// Agent SDK (osobny status).
    #[serde(rename = "sdk")]
    AgentSdk,
    /// Klucz API.
    #[serde(rename = "api")]
    Api,
}

/// Pewność źródła: V = strona źródłowa, W = źródło wtórne, ? = brak jednoznacznego zapisu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum Confidence {
    /// Potwierdzone na stronie źródłowej.
    #[serde(rename = "V")]
    Verified,
    /// Źródło wtórne.
    #[serde(rename = "W")]
    Secondary,
    /// Brak jednoznacznego zapisu.
    #[serde(rename = "?")]
    Unclear,
}

/// Źródło (cytat) uzasadniające status trasy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// URL albo znacznik „nieznany” z formatu rejestru.
    pub url: String,
    /// Cytat lub parafraza.
    pub quote: String,
    /// Data pobrania.
    pub retrieved_at: NaiveDate,
    /// Pewność.
    pub confidence: Confidence,
}

/// Wpis trasy w rejestrze.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryRoute {
    /// Identyfikator trasy.
    pub id: RouteId,
    /// Dostawca (id z `providers`).
    pub provider: String,
    /// Tryb.
    pub mode: RouteMode,
    /// Status zadeklarowany.
    pub status: RouteStatus,
    /// Data weryfikacji.
    pub verified_at: NaiveDate,
    /// Źródła.
    pub sources: Vec<Source>,
    /// Tagi prywatności trasy.
    pub privacy_tags: BTreeSet<PrivacyTag>,
    /// Co wolno.
    pub allowed: Vec<String>,
    /// Czego nie wolno.
    pub forbidden: Vec<String>,
    /// Przypięta wersja CLI (F4); `None` = nieprzypięta.
    pub cli_pinned_version: Option<String>,
    /// Czy trasa jest włączona domyślnie.
    pub enabled_by_default: bool,
}

/// Tagi dostawcy w rejestrze (źródło prawdy dla Routera).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryProvider {
    /// Identyfikator dostawcy (jak w katalogu).
    pub id: String,
    /// Jurysdykcja.
    pub jurisdiction: Jurisdiction,
    /// Tagi prywatności.
    pub privacy_tags: BTreeSet<PrivacyTag>,
    /// Pewność.
    pub confidence: Confidence,
    /// Uwagi.
    pub notes: String,
    /// Domeny webowego UI dostawcy dopisywane do deny-listy (F4; opcjonalne).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny_domains: Vec<String>,
}

/// Cały rejestr zgodności.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// Wersja formatu (patrz [`SUPPORTED_SCHEMA_VERSIONS`]).
    pub schema_version: u32,
    /// Po ilu dniach wpis jest nieświeży.
    pub max_age_days: u32,
    /// Data wygenerowania.
    pub generated_at: NaiveDate,
    /// Uwagi.
    pub notes: String,
    /// Trasy.
    pub routes: Vec<RegistryRoute>,
    /// Dostawcy (tagi).
    pub providers: Vec<RegistryProvider>,
}

/// Błędy wczytywania rejestru.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Błąd składni JSON lub brak pola.
    #[error("błąd składni rejestru: {0}")]
    Syntax(String),
    /// Nieobsługiwana wersja formatu.
    #[error("nieobsługiwana wersja rejestru {0}")]
    UnsupportedVersion(u32),
    /// Zduplikowany identyfikator.
    #[error("zduplikowany identyfikator `{0}`")]
    Duplicate(String),
    /// Trasa wskazuje nieznanego dostawcę.
    #[error("trasa `{route}` wskazuje nieznanego dostawcę `{provider}`")]
    UnknownProvider {
        /// Trasa.
        route: String,
        /// Dostawca.
        provider: String,
    },
    /// Niespójny wpis.
    #[error("niepoprawny wpis `{id}`: {reason}")]
    Invalid {
        /// Wpis.
        id: String,
        /// Powód.
        reason: String,
    },
}

impl Registry {
    /// Parsuje i waliduje rejestr z tekstu JSON.
    pub fn from_json(text: &str) -> Result<Self, RegistryError> {
        let version: serde_json::Value =
            serde_json::from_str(text).map_err(|e| RegistryError::Syntax(e.to_string()))?;
        let found = version
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| RegistryError::Syntax("brak pola `schema_version`".into()))?;
        let found = u32::try_from(found).unwrap_or(u32::MAX);
        if !SUPPORTED_SCHEMA_VERSIONS.contains(&found) {
            return Err(RegistryError::UnsupportedVersion(found));
        }
        let registry: Registry =
            serde_json::from_value(version).map_err(|e| RegistryError::Syntax(e.to_string()))?;
        registry.validate()?;
        Ok(registry)
    }

    /// Reguły semantyczne (poza tym, co wymusza serde).
    pub fn validate(&self) -> Result<(), RegistryError> {
        if !SUPPORTED_SCHEMA_VERSIONS.contains(&self.schema_version) {
            return Err(RegistryError::UnsupportedVersion(self.schema_version));
        }
        let invalid = |id: &str, reason: &str| RegistryError::Invalid {
            id: id.to_owned(),
            reason: reason.to_owned(),
        };
        if self.max_age_days == 0 {
            return Err(invalid("max_age_days", "musi być > 0"));
        }
        let mut providers = BTreeSet::new();
        for p in &self.providers {
            if !is_kebab(&p.id) {
                return Err(invalid(&p.id, "id dostawcy nie jest w kebab-case"));
            }
            if !providers.insert(p.id.as_str()) {
                return Err(RegistryError::Duplicate(p.id.clone()));
            }
        }
        let mut routes = BTreeSet::new();
        for r in &self.routes {
            if !routes.insert(r.id.as_str()) {
                return Err(RegistryError::Duplicate(r.id.to_string()));
            }
            if r.id.api_provider().is_some() {
                return Err(invalid(r.id.as_str(), "sufiks `.api` jest zarezerwowany"));
            }
            if !providers.contains(r.provider.as_str()) {
                return Err(RegistryError::UnknownProvider {
                    route: r.id.to_string(),
                    provider: r.provider.clone(),
                });
            }
            if r.sources.is_empty() {
                return Err(invalid(r.id.as_str(), "brak źródeł"));
            }
            if r.status == RouteStatus::Forbidden && r.enabled_by_default {
                return Err(invalid(
                    r.id.as_str(),
                    "zabroniona trasa włączona domyślnie",
                ));
            }
        }
        Ok(())
    }

    /// Wpis trasy po identyfikatorze.
    pub fn route(&self, id: &RouteId) -> Option<&RegistryRoute> {
        self.routes.iter().find(|r| &r.id == id)
    }

    /// Dostawcy według identyfikatora.
    pub fn providers_by_id(&self) -> BTreeMap<&str, &RegistryProvider> {
        self.providers.iter().map(|p| (p.id.as_str(), p)).collect()
    }
}

/// JSON Schema formatu rejestru (generowana z typów).
pub fn registry_schema() -> schemars::Schema {
    schemars::schema_for!(Registry)
}
