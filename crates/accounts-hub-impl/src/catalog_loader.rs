//! Ładowanie katalogu `providers-catalog/*.toml` z walidacją względem `schema.json`
//! (TOML → JSON → JSON Schema 2020-12, potem model i reguły semantyczne z kontraktu).

use std::path::Path;

use accounts_hub_contract::{CatalogError, ProviderCatalogEntry};

/// Schemat wpisu katalogu wbudowany z repo (źródło prawdy dla CI i dla runtime).
pub const CATALOG_SCHEMA_JSON: &str = include_str!("../../../providers-catalog/schema.json");

/// Walidator wpisów katalogu (skompilowany schemat).
pub struct CatalogValidator {
    validator: jsonschema::Validator,
}

impl CatalogValidator {
    /// Kompiluje schemat wbudowany.
    pub fn builtin() -> Result<Self, CatalogError> {
        Self::from_schema_str(CATALOG_SCHEMA_JSON)
    }

    /// Kompiluje schemat z tekstu JSON (walidacja formatów włączona: `uri` itd.).
    pub fn from_schema_str(schema: &str) -> Result<Self, CatalogError> {
        let schema_err = |reason: String| CatalogError::Schema {
            file: "schema.json".into(),
            reason,
        };
        let value: serde_json::Value =
            serde_json::from_str(schema).map_err(|e| schema_err(e.to_string()))?;
        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&value)
            .map_err(|e| schema_err(e.to_string()))?;
        Ok(Self { validator })
    }

    /// Waliduje tekst TOML względem schematu (wszystkie błędy w jednym komunikacie).
    pub fn validate_toml(&self, text: &str, file: &str) -> Result<(), CatalogError> {
        let table: toml::Table = toml::from_str(text).map_err(|e| CatalogError::Syntax {
            file: file.to_owned(),
            reason: e.message().to_owned(),
        })?;
        let json = serde_json::to_value(table).map_err(|e| CatalogError::Syntax {
            file: file.to_owned(),
            reason: e.to_string(),
        })?;
        let errors: Vec<String> = self
            .validator
            .iter_errors(&json)
            .map(|e| format!("{}: {}", e.instance_path(), e))
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(CatalogError::Schema {
                file: file.to_owned(),
                reason: errors.join("; "),
            })
        }
    }

    /// Schemat + model + reguły semantyczne dla jednego pliku.
    pub fn parse_entry(
        &self,
        text: &str,
        file_stem: &str,
    ) -> Result<ProviderCatalogEntry, CatalogError> {
        self.validate_toml(text, &format!("{file_stem}.toml"))?;
        ProviderCatalogEntry::from_toml(text, file_stem)
    }

    /// Wczytuje wszystkie `*.toml` z katalogu (posortowane po nazwie). Zwraca wpisy poprawne
    /// i listę błędów — zepsuty wpis nie blokuje pozostałych dostawców.
    pub fn load_dir(
        &self,
        dir: &Path,
    ) -> Result<(Vec<ProviderCatalogEntry>, Vec<CatalogError>), CatalogError> {
        let io = |reason: String| CatalogError::Io {
            file: dir.display().to_string(),
            reason,
        };
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| io(e.to_string()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        files.sort();
        let mut entries = Vec::new();
        let mut errors = Vec::new();
        for path in files {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let parsed = std::fs::read_to_string(&path)
                .map_err(|e| CatalogError::Io {
                    file: path.display().to_string(),
                    reason: e.to_string(),
                })
                .and_then(|text| self.parse_entry(&text, &stem));
            match parsed {
                Ok(entry) => entries.push(entry),
                Err(e) => errors.push(e),
            }
        }
        Ok((entries, errors))
    }
}
