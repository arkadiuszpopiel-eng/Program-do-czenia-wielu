//! Atrapa konfiguracji warstwowej (docs/PLAN.md §4.5, SPEC core-config „Fake”).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use core_config_contract::{
    ConfigChange, ConfigError, ConfigKey, ConfigLayer, ConfigStore, ConfigValue, ConfigWatch,
    MachineId, Origin, Scope, authorize,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_stream::wrappers::UnboundedReceiverStream;

type Flat = BTreeMap<ConfigKey, ConfigValue>;

/// Klucz zakresu (Scope nie ma porządku): (0 = global, 1 = sesja, 2 = agentka, id).
type ScopeKey = (u8, String);

fn scope_key(scope: &Scope) -> ScopeKey {
    match scope {
        Scope::Global => (0, String::new()),
        Scope::Session(id) => (1, id.as_str().to_owned()),
        Scope::Agent(id) => (2, id.as_str().to_owned()),
    }
}

/// Zapis zarejestrowany przez atrapę (historia bez git).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeRevision {
    /// Klucz.
    pub key: ConfigKey,
    /// Zakres.
    pub scope: Scope,
    /// Warstwa.
    pub layer: ConfigLayer,
    /// Wartość w warstwie przed zmianą.
    pub old: Option<ConfigValue>,
    /// Wartość w warstwie po zmianie.
    pub new: Option<ConfigValue>,
    /// Inicjator.
    pub origin: Origin,
}

#[derive(Default)]
struct State {
    defaults: Flat,
    /// (maszyna?, zakres) → mapa.
    layers: BTreeMap<(bool, ScopeKey), Flat>,
    history: Vec<FakeRevision>,
    fail_next: Option<ConfigError>,
    watchers: Vec<(String, UnboundedSender<ConfigChange>)>,
}

impl State {
    fn resolve(&self, key: &ConfigKey, scope: &Scope) -> Option<ConfigValue> {
        let sk = scope_key(scope);
        let global = scope_key(&Scope::Global);
        let mut order = Vec::with_capacity(4);
        if sk != global {
            order.push((true, sk.clone()));
            order.push((false, sk));
        }
        order.push((true, global.clone()));
        order.push((false, global));
        order
            .iter()
            .find_map(|slot| self.layers.get(slot).and_then(|f| f.get(key)))
            .or_else(|| self.defaults.get(key))
            .cloned()
    }

    fn notify(&mut self, change: &ConfigChange) {
        self.watchers.retain(|(prefix, tx)| {
            !change.key.has_prefix(prefix) || tx.send(change.clone()).is_ok()
        });
    }
}

/// `ConfigStore` w pamięci z tymi samymi regułami warstw i zakresów co implementacja.
pub struct FakeConfigStore {
    machine: MachineId,
    state: Mutex<State>,
}

impl FakeConfigStore {
    /// Pusta konfiguracja bieżącej maszyny.
    pub fn new(machine: MachineId) -> Self {
        Self {
            machine,
            state: Mutex::new(State::default()),
        }
    }

    /// Ustawia wartości domyślne (jak ze schematów modułów) — builder.
    #[must_use]
    pub fn with_defaults(
        self,
        defaults: impl IntoIterator<Item = (ConfigKey, ConfigValue)>,
    ) -> Self {
        self.lock().defaults.extend(defaults);
        self
    }

    /// Historia udanych zapisów (`set`), od najstarszego.
    pub fn history(&self) -> Vec<FakeRevision> {
        self.lock().history.clone()
    }

    /// Następny `set` zwróci ten błąd (jednorazowo, po sprawdzeniu reguły `kernel.*`).
    pub fn fail_next_set(&self, error: ConfigError) {
        self.lock().fail_next = Some(error);
    }

    /// Symuluje edycję pliku warstwy (zakres globalny): zastępuje warstwę treścią TOML i zwraca
    /// zmiany wartości wynikowych (wysłane też obserwatorom). Zmiana `kernel.*` → błąd.
    pub fn simulate_file_change(
        &self,
        layer: &ConfigLayer,
        toml_text: &str,
    ) -> Result<Vec<ConfigChange>, String> {
        let machine = self.writable(layer).map_err(|e| e.to_string())?;
        let table: toml::Table = toml::from_str(toml_text).map_err(|e| e.to_string())?;
        let mut flat = Flat::new();
        for (name, value) in &table {
            flatten(name, value, &mut flat)?;
        }
        let mut st = self.lock();
        let slot = (machine, scope_key(&Scope::Global));
        let old_layer = st.layers.get(&slot).cloned().unwrap_or_default();
        let changed: BTreeSet<ConfigKey> = old_layer
            .keys()
            .chain(flat.keys())
            .filter(|k| old_layer.get(*k) != flat.get(*k))
            .cloned()
            .collect();
        if let Some(k) = changed.iter().find(|k| k.is_kernel_policy()) {
            return Err(ConfigError::KernelPolicy(k.clone()).to_string());
        }
        let before: Vec<Option<ConfigValue>> = changed
            .iter()
            .map(|k| st.resolve(k, &Scope::Global))
            .collect();
        st.layers.insert(slot, flat);
        let mut changes = Vec::new();
        for (key, old) in changed.into_iter().zip(before) {
            let new = st.resolve(&key, &Scope::Global);
            if old != new {
                let change = ConfigChange {
                    key,
                    scope: Scope::Global,
                    layer: layer.clone(),
                    old,
                    new,
                    origin: Origin::User,
                };
                st.notify(&change);
                changes.push(change);
            }
        }
        Ok(changes)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `true` = nakładka maszyny, `false` = warstwa wspólna.
    fn writable(&self, layer: &ConfigLayer) -> Result<bool, ConfigError> {
        match layer {
            ConfigLayer::Default => Err(ConfigError::Persist(
                "warstwa Default jest tylko do odczytu".into(),
            )),
            ConfigLayer::Shared => Ok(false),
            ConfigLayer::Machine(id) if *id == self.machine => Ok(true),
            ConfigLayer::Machine(id) => Err(ConfigError::Persist(format!(
                "nakładka maszyny `{}` nie należy do tej maszyny",
                id.0
            ))),
        }
    }
}

fn flatten(path: &str, value: &toml::Value, out: &mut Flat) -> Result<(), String> {
    if let toml::Value::Table(table) = value {
        for (name, sub) in table {
            flatten(&format!("{path}.{name}"), sub, out)?;
        }
        return Ok(());
    }
    let key = ConfigKey::new(path).map_err(|e| e.to_string())?;
    out.insert(key, to_json(value));
    Ok(())
}

/// TOML → JSON (daty jako tekst).
fn to_json(value: &toml::Value) -> ConfigValue {
    match value {
        toml::Value::String(s) => ConfigValue::String(s.clone()),
        toml::Value::Integer(i) => ConfigValue::from(*i),
        toml::Value::Float(f) => ConfigValue::from(*f),
        toml::Value::Boolean(b) => ConfigValue::Bool(*b),
        toml::Value::Datetime(d) => ConfigValue::String(d.to_string()),
        toml::Value::Array(items) => ConfigValue::Array(items.iter().map(to_json).collect()),
        toml::Value::Table(t) => {
            ConfigValue::Object(t.iter().map(|(k, v)| (k.clone(), to_json(v))).collect())
        }
    }
}

#[async_trait]
impl ConfigStore for FakeConfigStore {
    async fn get(
        &self,
        key: &ConfigKey,
        scope: &Scope,
    ) -> Result<Option<ConfigValue>, ConfigError> {
        Ok(self.lock().resolve(key, scope))
    }

    async fn set(
        &self,
        key: &ConfigKey,
        value: Option<ConfigValue>,
        scope: &Scope,
        layer: &ConfigLayer,
        origin: Origin,
    ) -> Result<(), ConfigError> {
        authorize(key, &origin)?;
        let machine = self.writable(layer)?;
        if let Some(v) = &value
            && (v.is_null() || v.is_object())
        {
            return Err(ConfigError::SchemaViolation {
                key: key.clone(),
                reason: "`null` i obiekty niedozwolone".into(),
            });
        }
        let mut st = self.lock();
        if let Some(err) = st.fail_next.take() {
            return Err(err);
        }
        let old_effective = st.resolve(key, scope);
        let flat = st.layers.entry((machine, scope_key(scope))).or_default();
        let old_raw = flat.get(key).cloned();
        if old_raw == value {
            return Ok(());
        }
        match &value {
            Some(v) => flat.insert(key.clone(), v.clone()),
            None => flat.remove(key),
        };
        st.history.push(FakeRevision {
            key: key.clone(),
            scope: scope.clone(),
            layer: layer.clone(),
            old: old_raw,
            new: value,
            origin: origin.clone(),
        });
        let new_effective = st.resolve(key, scope);
        if old_effective != new_effective {
            let change = ConfigChange {
                key: key.clone(),
                scope: scope.clone(),
                layer: layer.clone(),
                old: old_effective,
                new: new_effective,
                origin,
            };
            st.notify(&change);
        }
        Ok(())
    }

    fn watch(&self, prefix: &str) -> ConfigWatch {
        let (tx, rx) = unbounded_channel();
        self.lock().watchers.push((prefix.to_owned(), tx));
        Box::pin(UnboundedReceiverStream::new(rx))
    }
}
