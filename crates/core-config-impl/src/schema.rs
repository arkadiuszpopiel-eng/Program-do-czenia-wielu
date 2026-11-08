//! Schematy konfiguracji modułów (JSON Schema, crate `jsonschema` bez pobierania zdalnych `$ref`).

use core_config_contract::{ConfigKey, ConfigValue};
use jsonschema::Validator;
use serde_json::{Map, Value};

use crate::layer::Flat;

/// Schemat poddrzewa kluczy `prefix.*`.
pub struct SchemaEntry {
    /// Prefiks poddrzewa (np. `voice.tts`).
    pub prefix: ConfigKey,
    validator: Validator,
    defaults: Flat,
}

impl SchemaEntry {
    /// Waliduje obiekt poddrzewa (klucze względne wobec prefiksu).
    pub fn validate(&self, instance: &Value) -> Result<(), String> {
        let errors: Vec<String> = self
            .validator
            .iter_errors(instance)
            .take(3)
            .map(|e| {
                let at = e.instance_path().to_string();
                if at.is_empty() {
                    e.to_string()
                } else {
                    format!("{at}: {e}")
                }
            })
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

/// Zbiór zarejestrowanych schematów.
#[derive(Default)]
pub struct SchemaSet {
    entries: Vec<SchemaEntry>,
}

impl SchemaSet {
    /// Rejestruje (lub zastępuje) schemat prefiksu. Wartości `default` stają się warstwą Default.
    pub fn register(&mut self, prefix: ConfigKey, schema: &Value) -> Result<(), String> {
        let validator =
            jsonschema::validator_for(schema).map_err(|e| format!("niepoprawny schemat: {e}"))?;
        let mut defaults = Flat::new();
        collect_defaults(prefix.as_str(), schema, &mut defaults)?;
        let entry = SchemaEntry {
            prefix,
            validator,
            defaults,
        };
        self.entries.retain(|e| e.prefix != entry.prefix);
        self.entries.push(entry);
        Ok(())
    }

    /// Scalone wartości domyślne wszystkich schematów.
    pub fn defaults(&self) -> Flat {
        self.entries
            .iter()
            .flat_map(|e| e.defaults.iter().map(|(k, v)| (k.clone(), v.clone())))
            .collect()
    }

    /// Schematy obejmujące klucz.
    pub fn covering<'a>(&'a self, key: &'a ConfigKey) -> impl Iterator<Item = &'a SchemaEntry> {
        self.entries
            .iter()
            .filter(move |e| key.has_prefix(e.prefix.as_str()))
    }

    /// Wszystkie schematy.
    pub fn entries(&self) -> &[SchemaEntry] {
        &self.entries
    }
}

fn collect_defaults(path: &str, schema: &Value, out: &mut Flat) -> Result<(), String> {
    let Some(obj) = schema.as_object() else {
        return Ok(());
    };
    if let Some(default) = obj.get("default") {
        flatten_default(path, default, out)?;
    }
    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        for (name, sub) in props {
            collect_defaults(&format!("{path}.{name}"), sub, out)?;
        }
    }
    Ok(())
}

fn flatten_default(path: &str, value: &ConfigValue, out: &mut Flat) -> Result<(), String> {
    match value {
        Value::Object(map) => flatten_object(path, map, out),
        Value::Null => Ok(()),
        leaf => {
            let key = ConfigKey::new(path).map_err(|e| format!("wartość domyślna: {e}"))?;
            out.insert(key, leaf.clone());
            Ok(())
        }
    }
}

fn flatten_object(path: &str, map: &Map<String, Value>, out: &mut Flat) -> Result<(), String> {
    for (name, value) in map {
        flatten_default(&format!("{path}.{name}"), value, out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_and_validation() {
        let mut set = SchemaSet::default();
        let schema = json!({
            "type": "object",
            "properties": {
                "engine": {"type": "string", "enum": ["piper", "pocket"], "default": "piper"},
                "limits": {"type": "object", "default": {"rate": 1}, "properties": {
                    "pitch": {"type": "number", "default": 0.5}
                }},
                "note": {"type": "string", "default": null}
            },
            "additionalProperties": false
        });
        let prefix = ConfigKey::new("voice.tts").unwrap();
        set.register(prefix.clone(), &schema).unwrap();
        let d = set.defaults();
        assert_eq!(d.len(), 3);
        assert_eq!(
            d[&ConfigKey::new("voice.tts.limits.rate").unwrap()],
            json!(1)
        );
        let engine = ConfigKey::new("voice.tts.engine").unwrap();
        let entry = set.covering(&engine).next().unwrap();
        assert!(entry.validate(&json!({"engine": "piper"})).is_ok());
        let err = entry
            .validate(&json!({"engine": "x", "extra": 1}))
            .unwrap_err();
        assert!(err.contains("/engine") || err.contains("extra"), "{err}");
        assert!(
            set.covering(&ConfigKey::new("voice.stt.x").unwrap())
                .next()
                .is_none()
        );
        set.register(prefix, &json!({"type": "object"})).unwrap();
        assert_eq!(set.entries().len(), 1, "ponowna rejestracja zastępuje");
    }

    #[test]
    fn rejects_bad_schema_and_secret_defaults() {
        let mut set = SchemaSet::default();
        let p = ConfigKey::new("x").unwrap();
        assert!(set.register(p.clone(), &json!({"type": 12})).is_err());
        let secret = json!({"properties": {"api_key": {"default": "sk-x"}}});
        assert!(set.register(p, &secret).is_err());
    }
}
