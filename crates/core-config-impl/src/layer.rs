//! Plik warstwy TOML ↔ płaskie mapy kluczy (zakres globalny + nadpisania sesji/agentek).
//!
//! Tabele zastrzeżone `"@session"."<id>"` i `"@agent"."<id>"` trzymają nadpisania zakresów
//! (`@` nie może wystąpić w `ConfigKey`, więc nie koliduje z kluczami modułów).

use std::collections::BTreeMap;

use core_bus_contract::{AgentId, SessionId};
use core_config_contract::{ConfigKey, ConfigValue, Scope};
use serde_json::Value;

/// Płaska mapa: klucz-liść → wartość.
pub type Flat = BTreeMap<ConfigKey, ConfigValue>;

/// Tabela nadpisań sesji.
pub const SESSION_TABLE: &str = "@session";
/// Tabela nadpisań agentek.
pub const AGENT_TABLE: &str = "@agent";

/// Zawartość jednego pliku warstwy.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerData {
    /// Zakres globalny.
    pub global: Flat,
    /// Nadpisania per sesja.
    pub sessions: BTreeMap<SessionId, Flat>,
    /// Nadpisania per agentka.
    pub agents: BTreeMap<AgentId, Flat>,
}

impl LayerData {
    /// Parsuje TOML; każdy klucz musi być poprawnym `ConfigKey` (sekrety odrzucane).
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| e.message().to_owned())?;
        let mut data = Self::default();
        for (name, value) in table {
            match name.as_str() {
                SESSION_TABLE => {
                    for (id, sub) in scoped_tables(&name, value)? {
                        data.sessions.insert(SessionId::new(id), flatten_root(sub)?);
                    }
                }
                AGENT_TABLE => {
                    for (id, sub) in scoped_tables(&name, value)? {
                        data.agents.insert(AgentId::new(id), flatten_root(sub)?);
                    }
                }
                _ => flatten(&name, &value, &mut data.global)?,
            }
        }
        Ok(data)
    }

    /// Zapis TOML (klucze posortowane — deterministyczny).
    pub fn render(&self) -> Result<String, String> {
        let mut root = unflatten(&self.global)?;
        let mut put = |name: &str, scoped: Vec<(String, &Flat)>| -> Result<(), String> {
            let mut outer = toml::Table::new();
            for (id, flat) in scoped.into_iter().filter(|(_, f)| !f.is_empty()) {
                outer.insert(id, toml::Value::Table(unflatten(flat)?));
            }
            if !outer.is_empty() {
                root.insert(name.to_owned(), toml::Value::Table(outer));
            }
            Ok(())
        };
        put(
            SESSION_TABLE,
            self.sessions
                .iter()
                .map(|(k, v)| (k.0.clone(), v))
                .collect(),
        )?;
        put(
            AGENT_TABLE,
            self.agents.iter().map(|(k, v)| (k.0.clone(), v)).collect(),
        )?;
        toml::to_string(&root).map_err(|e| e.to_string())
    }

    /// Mapa zakresu (jeśli istnieje).
    pub fn flat(&self, scope: &Scope) -> Option<&Flat> {
        match scope {
            Scope::Global => Some(&self.global),
            Scope::Session(id) => self.sessions.get(id),
            Scope::Agent(id) => self.agents.get(id),
        }
    }

    /// Mapa zakresu do zapisu (tworzona w razie potrzeby).
    pub fn flat_mut(&mut self, scope: &Scope) -> &mut Flat {
        match scope {
            Scope::Global => &mut self.global,
            Scope::Session(id) => self.sessions.entry(id.clone()).or_default(),
            Scope::Agent(id) => self.agents.entry(id.clone()).or_default(),
        }
    }

    /// Zakresy obecne w pliku (zawsze z `Global`).
    pub fn scopes(&self) -> Vec<Scope> {
        let mut out = vec![Scope::Global];
        out.extend(self.sessions.keys().cloned().map(Scope::Session));
        out.extend(self.agents.keys().cloned().map(Scope::Agent));
        out
    }
}

fn scoped_tables(name: &str, value: toml::Value) -> Result<Vec<(String, toml::Table)>, String> {
    let toml::Value::Table(outer) = value else {
        return Err(format!("`{name}` musi być tabelą"));
    };
    outer
        .into_iter()
        .map(|(id, sub)| match sub {
            toml::Value::Table(t) => Ok((id, t)),
            _ => Err(format!("`{name}.{id}` musi być tabelą")),
        })
        .collect()
}

fn flatten_root(table: toml::Table) -> Result<Flat, String> {
    let mut out = Flat::new();
    for (name, value) in table {
        flatten(&name, &value, &mut out)?;
    }
    Ok(out)
}

fn flatten(path: &str, value: &toml::Value, out: &mut Flat) -> Result<(), String> {
    if let toml::Value::Table(table) = value {
        for (name, sub) in table {
            flatten(&format!("{path}.{name}"), sub, out)?;
        }
        return Ok(());
    }
    let key = ConfigKey::new(path).map_err(|e| e.to_string())?;
    out.insert(key, toml_to_json(value)?);
    Ok(())
}

fn unflatten(flat: &Flat) -> Result<toml::Table, String> {
    let mut root = toml::Table::new();
    for (key, value) in flat {
        let segments: Vec<&str> = key.segments().collect();
        let Some((leaf, parents)) = segments.split_last() else {
            continue;
        };
        let mut table = &mut root;
        for seg in parents {
            let entry = table
                .entry((*seg).to_owned())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            table = match entry {
                toml::Value::Table(t) => t,
                _ => return Err(format!("konflikt struktury przy kluczu `{key}`")),
            };
        }
        if table.contains_key(*leaf) {
            return Err(format!("konflikt struktury przy kluczu `{key}`"));
        }
        table.insert((*leaf).to_owned(), json_to_toml(value)?);
    }
    Ok(root)
}

/// TOML → JSON (daty jako tekst; tabele w tablicach jako obiekty).
pub fn toml_to_json(value: &toml::Value) -> Result<Value, String> {
    Ok(match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::from(*i),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .ok_or_else(|| "NaN/nieskończoność niedozwolone".to_owned())?,
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(items) => Value::Array(
            items
                .iter()
                .map(toml_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        toml::Value::Table(t) => Value::Object(
            t.iter()
                .map(|(k, v)| Ok((k.clone(), toml_to_json(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}

/// JSON → TOML (`null` i liczby > i64 niedozwolone).
pub fn json_to_toml(value: &Value) -> Result<toml::Value, String> {
    Ok(match value {
        Value::Null => return Err("`null` niedozwolone (TOML nie ma null)".into()),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => toml::Value::Integer(i),
            (None, Some(f)) if !n.is_u64() => toml::Value::Float(f),
            _ => return Err(format!("liczba {n} poza zakresem TOML")),
        },
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(items) => toml::Value::Array(
            items
                .iter()
                .map(json_to_toml)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Value::Object(map) => toml::Value::Table(
            map.iter()
                .map(|(k, v)| Ok((k.clone(), json_to_toml(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn key(s: &str) -> ConfigKey {
        ConfigKey::new(s).unwrap()
    }

    #[test]
    fn parse_render_round_trip_with_scopes() {
        let text = r#"
[voice.tts]
engine = "piper"
rate = 1.25
voices = ["ala", "ola"]

["@session".s1.voice.tts]
engine = "pocket"

["@agent"."Ala Ma Kota".ui]
theme = "ciemny"
"#;
        let data = LayerData::parse(text).unwrap();
        assert_eq!(data.global[&key("voice.tts.engine")], json!("piper"));
        assert_eq!(data.global[&key("voice.tts.rate")], json!(1.25));
        let s1 = data.flat(&Scope::Session(SessionId::from("s1"))).unwrap();
        assert_eq!(s1[&key("voice.tts.engine")], json!("pocket"));
        assert_eq!(data.scopes().len(), 3);
        let back = LayerData::parse(&data.render().unwrap()).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn rejects_secrets_bad_keys_and_bad_scopes() {
        assert!(LayerData::parse("[providers.x]\napi_key = \"sk\"").is_err());
        assert!(LayerData::parse("Voice = 1").is_err());
        assert!(LayerData::parse("\"@session\" = 1").is_err());
        assert!(LayerData::parse("[\"@agent\"]\nx = 1").is_err());
        assert!(LayerData::parse("= =").is_err());
    }

    #[test]
    fn value_conversions() {
        assert!(json_to_toml(&json!(null)).is_err());
        assert!(json_to_toml(&json!(u64::MAX)).is_err());
        assert_eq!(json_to_toml(&json!(-3)).unwrap(), toml::Value::Integer(-3));
        let nested = json!([{"a": [1, 2.5, true, "x"]}]);
        assert_eq!(
            toml_to_json(&json_to_toml(&nested).unwrap()).unwrap(),
            nested
        );
        let date: toml::Value = toml::from_str::<toml::Table>("d = 2026-01-01")
            .unwrap()
            .remove("d")
            .unwrap();
        assert_eq!(toml_to_json(&date).unwrap(), json!("2026-01-01"));
    }

    #[test]
    fn structural_conflict_is_reported() {
        let mut data = LayerData::default();
        data.global.insert(key("a.b"), json!(1));
        data.global.insert(key("a.b.c"), json!(2));
        assert!(data.render().is_err());
    }
}
