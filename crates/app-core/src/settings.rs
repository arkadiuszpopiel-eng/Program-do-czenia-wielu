//! Drzewo ustawień §15 (`data/settings-pages.json`, etykiety PL/EN) i jego JSON Schema dla
//! `core-config` (walidacja zapisu, wartości domyślne w warstwie Default). Docelowo strony
//! dostarczają manifesty modułów; w F1 katalog jest wspólny dla UI i rdzenia.

use std::collections::BTreeMap;

use core_config_contract::ConfigKey;
use serde_json::{Map, Value, json};

use crate::dto::{SettingControl, SettingDef, SettingsPageDef};
use crate::error::AppError;

const PAGES_JSON: &str = include_str!("../data/settings-pages.json");

/// Klucze pomocnicze rdzenia (poza drzewem UI).
pub mod keys {
    /// Onboarding ukończony.
    pub const ONBOARDING_DONE: &str = "general.onboarding_done";
    /// Układ okna (per maszyna).
    pub const LAYOUT: &str = "ui.layout";
    /// Aktywna sesja (per maszyna).
    pub const ACTIVE_SESSION: &str = "ui.active_session";
    /// Nadpisania skrótów.
    pub const SHORTCUTS: &str = "ui.shortcuts";
    /// Sesja „Szybkie pytania".
    pub const QUICK_SESSION: &str = "quick.session_id";
    /// Tryb sesji Szybkiego pytania.
    pub const QUICK_MODE: &str = "quick.session_mode";
    /// Domyślny profil modelu.
    pub const DEFAULT_PROFILE: &str = "models.default_profile";
    /// Automatyczny tytuł.
    pub const AUTO_TITLE: &str = "sessions.auto_title";
    /// Okno cofnięcia usunięcia sesji (s).
    pub const DELETE_UNDO: &str = "sessions.delete_undo_seconds";
    /// Język.
    pub const LOCALE: &str = "ui.locale";
    /// Limit miesięczny włączony.
    pub const COST_LIMIT_ENABLED: &str = "costs.limit_enabled";
    /// Limit miesięczny w groszach.
    pub const COST_LIMIT_GROSZE: &str = "costs.limit_grosze";
    /// Poziom autonomii globalny.
    pub const AUTONOMY: &str = "permissions.global_level";
}

/// Katalog ustawień.
pub struct SettingsCatalog {
    pages: Vec<SettingsPageDef>,
    defs: BTreeMap<String, SettingDef>,
}

impl SettingsCatalog {
    /// Wczytuje wbudowany katalog.
    pub fn builtin() -> Result<Self, AppError> {
        let pages: Vec<SettingsPageDef> = serde_json::from_str(PAGES_JSON)
            .map_err(|e| AppError::internal(format!("katalog ustawień: {e}")))?;
        let defs = pages
            .iter()
            .flat_map(|p| p.settings.iter())
            .map(|d| (d.key.clone(), d.clone()))
            .collect();
        Ok(Self { pages, defs })
    }

    /// Strony (drzewo §15).
    pub fn pages(&self) -> &[SettingsPageDef] {
        &self.pages
    }

    /// Definicja klucza.
    pub fn def(&self, key: &str) -> Option<&SettingDef> {
        self.defs.get(key)
    }

    /// Wszystkie definicje.
    pub fn defs(&self) -> impl Iterator<Item = &SettingDef> {
        self.defs.values()
    }

    /// JSON Schema per prefiks (pierwszy segment klucza) do `FileConfigStore::register_schema`.
    pub fn schemas(&self) -> Result<Vec<(ConfigKey, Value)>, AppError> {
        let mut by_prefix: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
        for def in self.defs.values() {
            let Some((prefix, rest)) = def.key.split_once('.') else {
                continue;
            };
            by_prefix
                .entry(prefix.to_owned())
                .or_default()
                .insert(rest.to_owned(), value_schema(def));
        }
        by_prefix
            .into_iter()
            .map(|(prefix, props)| {
                let key = ConfigKey::new(prefix)?;
                Ok((key, json!({ "type": "object", "properties": props })))
            })
            .collect()
    }
}

fn value_schema(def: &SettingDef) -> Value {
    let default = def.default.to_json();
    match &def.control {
        SettingControl::Toggle => json!({ "type": "boolean", "default": default }),
        SettingControl::Select { options } => {
            let values: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
            json!({ "type": "string", "enum": values, "default": default })
        }
        SettingControl::Number { min, max, .. } => {
            json!({ "type": "number", "minimum": min, "maximum": max, "default": default })
        }
        SettingControl::Text => json!({ "type": "string", "maxLength": 4096, "default": default }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::SettingValue;

    #[test]
    fn builtin_catalog_has_pages_defaults_and_schemas() {
        let c = SettingsCatalog::builtin().unwrap();
        assert!(c.pages().len() >= 20);
        assert!(c.def("ui.theme").is_some());
        assert_eq!(
            c.def("general.destroy_webview_after")
                .map(|d| d.default.clone()),
            Some(SettingValue::Number(10.into()))
        );
        let schemas = c.schemas().unwrap();
        let ui = schemas.iter().find(|(k, _)| k.as_str() == "ui").unwrap();
        assert!(ui.1["properties"]["theme"]["enum"].is_array());
    }
}
