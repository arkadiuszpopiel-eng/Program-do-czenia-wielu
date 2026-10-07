//! Katalog dostawców (`providers-catalog/*.toml`) wbudowany w binarium: ten sam tekst trafia do
//! `accounts-hub` (walidacja JSON Schema, stan kont) i do `providers-api-impl` (budowa adaptera).

use std::collections::BTreeMap;

use accounts_hub_contract::ProviderCatalogEntry;
use accounts_hub_impl::CatalogValidator;
use providers_api_impl::CatalogEntry;

/// Pliki katalogu: (identyfikator = nazwa pliku, treść TOML).
pub const CATALOG_FILES: &[(&str, &str)] = &[
    (
        "anthropic",
        include_str!("../../../providers-catalog/anthropic.toml"),
    ),
    (
        "azure-speech",
        include_str!("../../../providers-catalog/azure-speech.toml"),
    ),
    (
        "cartesia",
        include_str!("../../../providers-catalog/cartesia.toml"),
    ),
    (
        "custom-anthropic-compatible",
        include_str!("../../../providers-catalog/custom-anthropic-compatible.toml"),
    ),
    (
        "custom-openai-compatible",
        include_str!("../../../providers-catalog/custom-openai-compatible.toml"),
    ),
    (
        "deepseek",
        include_str!("../../../providers-catalog/deepseek.toml"),
    ),
    (
        "elevenlabs",
        include_str!("../../../providers-catalog/elevenlabs.toml"),
    ),
    (
        "google",
        include_str!("../../../providers-catalog/google.toml"),
    ),
    ("kimi", include_str!("../../../providers-catalog/kimi.toml")),
    (
        "minimax",
        include_str!("../../../providers-catalog/minimax.toml"),
    ),
    (
        "mistral",
        include_str!("../../../providers-catalog/mistral.toml"),
    ),
    (
        "openai",
        include_str!("../../../providers-catalog/openai.toml"),
    ),
    (
        "openrouter",
        include_str!("../../../providers-catalog/openrouter.toml"),
    ),
    ("qwen", include_str!("../../../providers-catalog/qwen.toml")),
    (
        "soniox",
        include_str!("../../../providers-catalog/soniox.toml"),
    ),
    ("xai", include_str!("../../../providers-catalog/xai.toml")),
    ("zai", include_str!("../../../providers-catalog/zai.toml")),
];

/// Katalog w dwóch widokach (hub kont i adaptery dostawców).
#[derive(Debug, Clone, Default)]
pub struct ProviderCatalog {
    /// Wpisy dla `accounts-hub` (po walidacji schematu).
    pub hub: Vec<ProviderCatalogEntry>,
    /// Wpisy dla `providers-api-impl` po identyfikatorze.
    pub api: BTreeMap<String, CatalogEntry>,
}

impl ProviderCatalog {
    /// Wczytuje wbudowany katalog. Uszkodzony wpis nie blokuje pozostałych (ostrzeżenie w logu).
    pub fn builtin() -> Self {
        let validator = match CatalogValidator::builtin() {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::error!(error = %e, "schemat katalogu dostawców niepoprawny");
                None
            }
        };
        let mut catalog = Self::default();
        for (id, text) in CATALOG_FILES {
            if let Some(validator) = &validator {
                match validator.parse_entry(text, id) {
                    Ok(entry) => catalog.hub.push(entry),
                    Err(e) => tracing::warn!(dostawca = id, error = %e, "wpis katalogu pominięty"),
                }
            }
            match CatalogEntry::parse_toml(text) {
                Ok(entry) => {
                    catalog.api.insert((*id).to_owned(), entry);
                }
                Err(e) => tracing::warn!(dostawca = id, error = %e, "adapter katalogu pominięty"),
            }
        }
        catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_parses_every_file_in_both_views() {
        let catalog = ProviderCatalog::builtin();
        assert_eq!(catalog.hub.len(), CATALOG_FILES.len());
        assert_eq!(catalog.api.len(), CATALOG_FILES.len());
        assert!(catalog.api.contains_key("anthropic"));
        assert!(catalog.hub.iter().any(|e| e.id.as_str() == "openai"));
    }
}
