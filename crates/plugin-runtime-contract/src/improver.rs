//! Integracja z Ulepszaczem — pierścień R2 (PLAN §12.1): propozycja „zainstaluj/zaktualizuj
//! wtyczkę” to zmiana klucza `plugins.<id>.version` (lista `improver_contract::IMPROVABLE`),
//! której **wartością jest hash przejrzanej wersji**. Skrót zatwierdzenia Ulepszacza
//! (`Proposal::digest` = SHA-256 klucza, starej i nowej wartości) wiąże więc dokładne bajty
//! modułu i manifest; wdrożenie = [`crate::Plugins::deploy_r2`] z zatwierdzeniem właściciela
//! tego samego hasha. Ulepszacz nie instaluje niczego sam (brak portu zapisu plików).

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::PluginError;
use crate::manifest::PluginId;
use crate::model::PluginRecord;
use crate::validate::is_sha256_hex;

/// Prefiks kluczy R2.
pub const KEY_PREFIX: &str = "plugins.";
/// Sufiks kluczy R2.
pub const KEY_SUFFIX: &str = ".version";

/// Zmiana R2 do przekazania Ulepszaczowi (`ChangeTarget::Config { key, value }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct R2Change {
    /// Wtyczka.
    pub plugin: PluginId,
    /// Wersja docelowa.
    #[schemars(with = "String")]
    pub version: Version,
    /// Wersja obecnie zainstalowana (`None` = instalacja).
    #[schemars(with = "Option<String>")]
    pub from_version: Option<Version>,
    /// Klucz konfiguracji (`plugins.<id z _ zamiast ->.version`).
    pub key: String,
    /// Hash przejrzanej wersji (wartość klucza; 64 znaki hex).
    pub review_hash: String,
    /// Hash modułu Wasm (informacyjnie dla karty).
    pub wasm_sha256: String,
    /// Rodziny zdolności dodane względem zainstalowanej wersji (karta: „wtyczka chce więcej”).
    pub added_capabilities: Vec<String>,
}

impl R2Change {
    /// Wartość klucza dla `core-config`/Ulepszacza.
    pub fn value(&self) -> serde_json::Value {
        serde_json::Value::String(self.review_hash.clone())
    }
}

/// Klucz R2 wtyczki (segmenty `core-config` nie dopuszczają `-`, identyfikator nie ma `_`).
pub fn improver_key(id: &PluginId) -> String {
    format!("{KEY_PREFIX}{}{KEY_SUFFIX}", id.as_str().replace('-', "_"))
}

/// Zmiana R2 dla propozycji `record` (względem zainstalowanej `installed`).
pub fn r2_change(record: &PluginRecord, installed: Option<&PluginRecord>) -> R2Change {
    let before: Vec<String> = installed.map(|r| r.manifest.families()).unwrap_or_default();
    R2Change {
        plugin: record.manifest.id.clone(),
        version: record.manifest.version.clone(),
        from_version: installed.map(|r| r.manifest.version.clone()),
        key: improver_key(&record.manifest.id),
        review_hash: record.review_hash.clone(),
        wasm_sha256: record.manifest.wasm_sha256.clone(),
        added_capabilities: record
            .manifest
            .families()
            .into_iter()
            .filter(|f| !before.contains(f))
            .collect(),
    }
}

/// Odczyt zmiany R2 zatwierdzonej w Ulepszaczu: (wtyczka, hash przejrzanej wersji).
pub fn parse_r2(key: &str, value: &serde_json::Value) -> Result<(PluginId, String), PluginError> {
    let bad = |why: &str| PluginError::Invalid(format!("zmiana R2 `{key}`: {why}"));
    let middle = key
        .strip_prefix(KEY_PREFIX)
        .and_then(|k| k.strip_suffix(KEY_SUFFIX))
        .ok_or_else(|| bad("klucz spoza `plugins.<id>.version`"))?;
    if middle.contains('-') {
        return Err(bad("segment klucza z `-`"));
    }
    let id = PluginId::new(middle.replace('_', "-"));
    if !id.is_valid() {
        return Err(bad("niepoprawny identyfikator wtyczki"));
    }
    let hash = value
        .as_str()
        .filter(|h| is_sha256_hex(h))
        .ok_or_else(|| bad("wartość musi być hashem przejrzanej wersji"))?;
    Ok((id, hash.to_owned()))
}
