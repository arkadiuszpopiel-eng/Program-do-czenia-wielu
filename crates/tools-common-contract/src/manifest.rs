//! Manifest narzędzia (PLAN §8.7): nazwa, opis dla modelu, JSON Schema wejścia i wyjścia,
//! `reversible: yes|scoped|no`, wymagane zdolności, grupy ról, źródło niezaufanej treści.

use providers_contract::{ToolSpec, valid_tool_name};
use risk_classifier_contract::Reversibility;
use safety_broker_contract::TaintSource;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::{Deserialize, Serialize};

/// Manifest narzędzia — jedno źródło prawdy dla modelu (`ToolSpec`), Brokera (fakty) i UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolManifest {
    /// Nazwa dla modelu (`^[a-zA-Z0-9_-]{1,64}$`), np. `fs_read`.
    pub name: String,
    /// Identyfikator w faktach Brokera i zdarzeniach, np. `tools-fs.read`.
    pub id: String,
    /// Krótka nazwa dla UI (po polsku), np. „Odczyt pliku”.
    pub title: String,
    /// Opis dla modelu (po polsku): co robi, kiedy używać, ograniczenia.
    pub description: String,
    /// JSON Schema argumentów (obiekt, `additionalProperties: false`).
    pub input_schema: serde_json::Value,
    /// JSON Schema danych wyniku (`ToolOutcome::data`).
    pub output_schema: serde_json::Value,
    /// Odwracalność z manifestu (`yes|scoped|no`).
    pub reversible: Reversibility,
    /// Rodziny zdolności, o które narzędzie prosi Brokera (`fs.read`, `fs.write`, `shell.exec`…).
    pub capabilities: Vec<String>,
    /// Grupy ról (`personas`), którym narzędzie przysługuje (`fs`, `fs.read`, `shell`…).
    pub groups: Vec<String>,
    /// Czy zmienia stan (zapis, usunięcie, polecenie powłoki, zapis schowka).
    pub mutating: bool,
    /// Źródło niezaufanej treści, gdy wynik niesie dane z zewnątrz (taint sesji).
    pub untrusted_output: Option<TaintSource>,
}

/// Błąd manifestu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("manifest `{name}`: {reason}")]
pub struct ManifestError {
    /// Nazwa narzędzia.
    pub name: String,
    /// Powód.
    pub reason: String,
}

impl ToolManifest {
    /// Definicja narzędzia dla modelu (IR `providers-contract`).
    pub fn to_spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            strict: false,
        }
    }

    /// Czy narzędzie przysługuje roli o podanych grupach narzędzi (`Role::tools`).
    /// Grupa ogólna (`fs`) obejmuje szczegółowe (`fs.read`); rola tylko do odczytu nie dostaje
    /// narzędzi zmieniających stan.
    pub fn allowed_for(&self, role_groups: &[String], read_only: bool) -> bool {
        if read_only && self.mutating {
            return false;
        }
        role_groups.iter().any(|g| {
            self.groups
                .iter()
                .any(|mine| mine == g || mine.starts_with(&format!("{g}.")))
        })
    }

    /// Walidacja: nazwa, opis, schemat obiektowy zamknięty, zdolności i grupy niepuste.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let fail = |reason: &str| {
            Err(ManifestError {
                name: self.name.clone(),
                reason: reason.to_owned(),
            })
        };
        if !valid_tool_name(&self.name) {
            return fail("nazwa spoza `^[a-zA-Z0-9_-]{1,64}$`");
        }
        if self.description.trim().len() < 20 || self.title.trim().is_empty() {
            return fail("brak opisu dla modelu albo tytułu");
        }
        if self.id.trim().is_empty() || self.groups.is_empty() || self.capabilities.is_empty() {
            return fail("brak identyfikatora, grup ról albo zdolności");
        }
        let schema = &self.input_schema;
        if schema.get("type") != Some(&serde_json::json!("object")) {
            return fail("schemat wejścia musi być obiektem");
        }
        if schema.get("additionalProperties") != Some(&serde_json::json!(false)) {
            return fail("schemat wejścia musi mieć `additionalProperties: false`");
        }
        if !self.output_schema.is_object() {
            return fail("brak schematu wyjścia");
        }
        Ok(())
    }
}

/// JSON Schema typu jako samodzielny obiekt (podschematy wstawione, bez `$schema`/`title`) —
/// tak, jak oczekują dostawcy w definicji narzędzia.
pub fn schema_of<T: JsonSchema>() -> serde_json::Value {
    let schema = SchemaSettings::draft07()
        .with(|s| s.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<T>();
    let mut value = serde_json::to_value(schema).unwrap_or_default();
    if let Some(obj) = value.as_object_mut() {
        obj.remove("$schema");
        obj.remove("title");
        obj.remove("definitions");
        obj.remove("$defs");
    }
    compact_enums(&mut value);
    value
}

/// `oneOf` złożone wyłącznie z napisów `const` → zwięzłe `enum` (mniej tokenów dla małych
/// modeli lokalnych, prostsza gramatyka llama.cpp).
fn compact_enums(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(obj) => {
            let consts: Option<Vec<serde_json::Value>> = obj
                .get("oneOf")
                .and_then(serde_json::Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .map(|i| i.get("const").filter(|c| c.is_string()).cloned())
                        .collect()
                });
            if let Some(consts) = consts {
                obj.remove("oneOf");
                obj.insert("type".into(), "string".into());
                obj.insert("enum".into(), serde_json::Value::Array(consts));
            }
            obj.values_mut().for_each(compact_enums);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(compact_enums),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Argumenty testowe.
    #[derive(Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Args {
        /// Ścieżka.
        path: String,
        /// Limit.
        #[serde(default)]
        limit: Option<u32>,
    }

    fn manifest() -> ToolManifest {
        ToolManifest {
            name: "fs_read".into(),
            id: "tools-fs.read".into(),
            title: "Odczyt pliku".into(),
            description: "Czyta plik tekstowy w zakresie tokenu i zwraca jego treść.".into(),
            input_schema: schema_of::<Args>(),
            output_schema: serde_json::json!({"type": "object"}),
            reversible: Reversibility::Yes,
            capabilities: vec!["fs.read".into()],
            groups: vec!["fs".into(), "fs.read".into()],
            mutating: false,
            untrusted_output: Some(TaintSource::File),
        }
    }

    #[test]
    fn schema_is_closed_object_without_meta() {
        let s = schema_of::<Args>();
        assert_eq!(s["type"], "object");
        assert_eq!(s["additionalProperties"], false);
        assert_eq!(s["required"], serde_json::json!(["path"]));
        assert!(s.get("$schema").is_none() && s.get("title").is_none());
        assert!(manifest().validate().is_ok());
        assert_eq!(manifest().to_spec().name, "fs_read");
    }

    #[test]
    fn validation_rejects_bad_manifests() {
        let mut m = manifest();
        m.name = "zła nazwa".into();
        assert!(m.validate().is_err());
        let mut m = manifest();
        m.input_schema = serde_json::json!({"type": "object"});
        assert!(m.validate().is_err());
        let mut m = manifest();
        m.capabilities.clear();
        assert!(m.validate().is_err());
        let mut m = manifest();
        m.description = "krótko".into();
        assert!(m.validate().is_err());
        let mut m = manifest();
        m.input_schema = serde_json::json!({"type": "string"});
        assert!(m.validate().is_err());
        let mut m = manifest();
        m.output_schema = serde_json::Value::Null;
        assert!(m.validate().unwrap_err().to_string().contains("fs_read"));
    }

    #[test]
    fn role_groups() {
        let m = manifest();
        assert!(m.allowed_for(&["fs".into()], false));
        assert!(m.allowed_for(&["fs.read".into()], true));
        assert!(!m.allowed_for(&["shell".into()], false));
        let mut w = manifest();
        w.mutating = true;
        w.groups = vec!["fs".into(), "fs.write".into()];
        assert!(!w.allowed_for(&["fs".into()], true));
        assert!(!w.allowed_for(&["fs.read".into()], false));
        assert!(w.allowed_for(&["fs".into()], false));
    }
}
