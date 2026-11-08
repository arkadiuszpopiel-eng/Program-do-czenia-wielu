//! Model w pamięci: Default < Shared < Machine, nadpisania sesji/agentek ponad warstwami.

use std::collections::BTreeSet;

use core_config_contract::{ConfigKey, ConfigValue, Scope};
use serde_json::{Map, Value};

use crate::layer::{Flat, LayerData};

/// Warstwa zapisywalna (plik).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writable {
    /// `shared.toml`.
    Shared,
    /// `machine/<id>.toml` bieżącej maszyny.
    Machine,
}

/// Stan konfiguracji.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    /// Wartości domyślne ze schematów.
    pub defaults: Flat,
    /// Warstwa wspólna.
    pub shared: LayerData,
    /// Nakładka bieżącej maszyny.
    pub machine: LayerData,
}

impl Model {
    /// Plik warstwy.
    pub fn layer(&self, which: Writable) -> &LayerData {
        match which {
            Writable::Shared => &self.shared,
            Writable::Machine => &self.machine,
        }
    }

    /// Plik warstwy do zapisu.
    pub fn layer_mut(&mut self, which: Writable) -> &mut LayerData {
        match which {
            Writable::Shared => &mut self.shared,
            Writable::Machine => &mut self.machine,
        }
    }

    /// Kandydaci od najwyższego priorytetu: zakres(maszyna) > zakres(wspólna) > maszyna >
    /// wspólna > domyślna (SPEC: sesja/agentka > maszyna > wspólna > domyślna).
    fn candidates<'a>(&'a self, scope: &Scope) -> Vec<&'a Flat> {
        let mut out = Vec::with_capacity(5);
        if *scope != Scope::Global {
            out.extend(self.machine.flat(scope));
            out.extend(self.shared.flat(scope));
        }
        out.push(&self.machine.global);
        out.push(&self.shared.global);
        out.push(&self.defaults);
        out
    }

    /// Wartość wynikowa klucza w zakresie.
    pub fn resolve(&self, key: &ConfigKey, scope: &Scope) -> Option<&ConfigValue> {
        self.candidates(scope)
            .into_iter()
            .find_map(|flat| flat.get(key))
    }

    /// Klucze widoczne w zakresie.
    pub fn keys(&self, scope: &Scope) -> BTreeSet<&ConfigKey> {
        self.candidates(scope)
            .into_iter()
            .flat_map(|flat| flat.keys())
            .collect()
    }

    /// Wszystkie zakresy występujące w plikach (z `Global`, bez powtórzeń).
    pub fn scopes(&self) -> Vec<Scope> {
        let mut out: Vec<Scope> = Vec::new();
        for scope in self
            .shared
            .scopes()
            .into_iter()
            .chain(self.machine.scopes())
        {
            if !out.contains(&scope) {
                out.push(scope);
            }
        }
        out
    }

    /// Wynikowe poddrzewo pod `prefix` jako obiekt JSON (klucze względne) — wejście walidacji.
    /// Konflikty struktury między warstwami (liść vs tabela) rozstrzyga pierwszy wstawiony klucz.
    pub fn subtree(&self, prefix: &ConfigKey, scope: &Scope) -> Value {
        let depth = prefix.segments().count();
        let mut root = Map::new();
        for key in self.keys(scope) {
            if key == prefix || !key.has_prefix(prefix.as_str()) {
                continue;
            }
            let Some(value) = self.resolve(key, scope) else {
                continue;
            };
            let rel: Vec<&str> = key.segments().skip(depth).collect();
            insert_nested(&mut root, &rel, value.clone());
        }
        Value::Object(root)
    }
}

fn insert_nested(root: &mut Map<String, Value>, path: &[&str], value: Value) {
    let Some((first, rest)) = path.split_first() else {
        return;
    };
    if rest.is_empty() {
        root.entry((*first).to_owned()).or_insert(value);
        return;
    }
    let child = root
        .entry((*first).to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(map) = child {
        insert_nested(map, rest, value);
    }
}

/// Zmiany surowe (klucz → stara, nowa) między dwiema mapami.
pub fn raw_diff(
    old: Option<&Flat>,
    new: Option<&Flat>,
) -> Vec<(ConfigKey, Option<Value>, Option<Value>)> {
    let empty = Flat::new();
    let old = old.unwrap_or(&empty);
    let new = new.unwrap_or(&empty);
    let keys: BTreeSet<&ConfigKey> = old.keys().chain(new.keys()).collect();
    keys.into_iter()
        .filter(|k| old.get(*k) != new.get(*k))
        .map(|k| ((*k).clone(), old.get(k).cloned(), new.get(k).cloned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_bus_contract::SessionId;
    use serde_json::json;

    fn key(s: &str) -> ConfigKey {
        ConfigKey::new(s).unwrap()
    }

    #[test]
    fn resolution_order_and_subtree() {
        let mut m = Model::default();
        let s1 = Scope::Session(SessionId::from("s1"));
        m.defaults.insert(key("a.x"), json!(0));
        m.defaults.insert(key("a.y"), json!("d"));
        m.shared.global.insert(key("a.x"), json!(1));
        m.machine.global.insert(key("a.x"), json!(2));
        m.shared.flat_mut(&s1).insert(key("a.x"), json!(3));
        m.shared.global.insert(key("a.z.deep"), json!(true));
        assert_eq!(m.resolve(&key("a.x"), &Scope::Global), Some(&json!(2)));
        assert_eq!(m.resolve(&key("a.x"), &s1), Some(&json!(3)));
        assert_eq!(
            m.subtree(&key("a"), &s1),
            json!({"x": 3, "y": "d", "z": {"deep": true}})
        );
        assert_eq!(m.scopes().len(), 2);
        let mut n = m.clone();
        n.machine.global.remove(&key("a.x"));
        assert_eq!(n.resolve(&key("a.x"), &Scope::Global), Some(&json!(1)));
        let raw = raw_diff(Some(&m.machine.global), Some(&n.machine.global));
        assert_eq!(raw, vec![(key("a.x"), Some(json!(2)), None)]);
    }
}
