//! Przejścia stanu pod zamkiem: plan zapisu (`set`) i plan przeładowania plików.
//! Plan liczy kandydata bez efektów ubocznych; stan zmienia się dopiero po udanym zapisie pliku.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use core_config_contract::{
    ConfigChange, ConfigError, ConfigKey, ConfigLayer, ConfigValue, MachineId, Origin, Scope,
};

use crate::history::{ChangeSource, HistoryEntry};
use crate::layer::{Flat, LayerData, json_to_toml};
use crate::model::{Model, Writable, raw_diff};
use crate::schema::SchemaSet;

/// Błąd przeładowania plików (stan w pamięci zostaje bez zmian).
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ReloadError {
    /// Plik nie parsuje się albo zawiera niedozwolony klucz (np. sekret).
    #[error("plik {} jest niepoprawny: {reason}", file.display())]
    Invalid {
        /// Plik.
        file: PathBuf,
        /// Powód.
        reason: String,
    },
    /// Plik zmienia politykę Jądra (`kernel.*`) z pominięciem Brokera.
    #[error("plik zmienia politykę Jądra `{0}` z pominięciem Brokera")]
    KernelPolicy(ConfigKey),
    /// Wartości z plików naruszają schemat modułu.
    #[error("konfiguracja z plików narusza schemat `{prefix}`: {reason}")]
    Schema {
        /// Prefiks schematu.
        prefix: ConfigKey,
        /// Powód.
        reason: String,
    },
}

/// Stan chroniony zamkiem.
#[derive(Default)]
pub(crate) struct State {
    pub model: Model,
    pub schemas: SchemaSet,
    /// Pliki warstw uznane za niepoprawne — zapis do nich wstrzymany, by nie nadpisać edycji.
    pub broken_shared: bool,
    pub broken_machine: bool,
}

impl State {
    pub fn broken(&self, which: Writable) -> bool {
        match which {
            Writable::Shared => self.broken_shared,
            Writable::Machine => self.broken_machine,
        }
    }

    pub fn set_broken(&mut self, which: Writable, broken: bool) {
        match which {
            Writable::Shared => self.broken_shared = broken,
            Writable::Machine => self.broken_machine = broken,
        }
    }
}

/// Zaplanowany zapis.
pub(crate) struct SetPlan {
    pub which: Writable,
    pub text: String,
    pub candidate: Model,
    pub change: Option<ConfigChange>,
    pub entry: HistoryEntry,
}

/// Warstwa kontraktu → plik; Default i cudza maszyna są tylko do odczytu.
pub(crate) fn writable(layer: &ConfigLayer, machine: &MachineId) -> Result<Writable, ConfigError> {
    match layer {
        ConfigLayer::Default => Err(ConfigError::Persist(
            "warstwa Default jest tylko do odczytu (wartości domyślne pochodzą ze schematów)"
                .into(),
        )),
        ConfigLayer::Shared => Ok(Writable::Shared),
        ConfigLayer::Machine(id) if id == machine => Ok(Writable::Machine),
        ConfigLayer::Machine(id) => Err(ConfigError::Persist(format!(
            "nakładka maszyny `{}` nie należy do tej maszyny (`{}`)",
            id.0, machine.0
        ))),
    }
}

fn violation(key: &ConfigKey, reason: impl Into<String>) -> ConfigError {
    ConfigError::SchemaViolation {
        key: key.clone(),
        reason: reason.into(),
    }
}

/// Wartość musi być liściem reprezentowalnym w TOML (bez `null` i obiektów).
pub(crate) fn check_value(key: &ConfigKey, value: &ConfigValue) -> Result<(), ConfigError> {
    if value.is_object() {
        return Err(violation(key, "obiekt niedozwolony; ustaw klucze-liście"));
    }
    json_to_toml(value)
        .map(|_| ())
        .map_err(|e| violation(key, e))
}

fn structural_conflict(flat: &Flat, key: &ConfigKey) -> Option<ConfigKey> {
    flat.keys()
        .find(|k| *k != key && (k.has_prefix(key.as_str()) || key.has_prefix(k.as_str())))
        .cloned()
}

/// Wspólne dane wejściowe planów.
pub(crate) struct Ctx<'a> {
    pub machine: &'a MachineId,
    pub strict_keys: bool,
    pub now: DateTime<Utc>,
}

/// Plan zapisu; `Ok(None)` = wartość w warstwie już taka (brak zmiany i wpisu historii).
pub(crate) fn plan_set(
    st: &State,
    ctx: &Ctx<'_>,
    key: &ConfigKey,
    value: Option<ConfigValue>,
    scope: &Scope,
    which: Writable,
    origin: Origin,
) -> Result<Option<SetPlan>, ConfigError> {
    if st.broken(which) {
        return Err(ConfigError::Persist(
            "plik warstwy jest niepoprawny; popraw go i przeładuj przed zapisem".into(),
        ));
    }
    let mut candidate = st.model.clone();
    let flat = candidate.layer_mut(which).flat_mut(scope);
    let raw_old = flat.get(key).cloned();
    if raw_old == value {
        return Ok(None);
    }
    match &value {
        Some(v) => {
            if let Some(other) = structural_conflict(flat, key) {
                return Err(violation(
                    key,
                    format!("konflikt struktury z kluczem `{other}`"),
                ));
            }
            flat.insert(key.clone(), v.clone());
        }
        None => {
            flat.remove(key);
        }
    }
    let mut covered = false;
    for entry in st.schemas.covering(key) {
        covered = true;
        let instance = candidate.subtree(&entry.prefix, scope);
        entry.validate(&instance).map_err(|r| violation(key, r))?;
    }
    if ctx.strict_keys && !covered && value.is_some() {
        return Err(ConfigError::UnknownKey(key.clone()));
    }
    let text = candidate
        .layer(which)
        .render()
        .map_err(ConfigError::Persist)?;
    let layer = layer_of(which, ctx.machine);
    let old = st.model.resolve(key, scope).cloned();
    let new = candidate.resolve(key, scope).cloned();
    let change = (old != new).then(|| ConfigChange {
        key: key.clone(),
        scope: scope.clone(),
        layer: layer.clone(),
        old,
        new,
        origin: origin.clone(),
    });
    let entry = HistoryEntry {
        ts: ctx.now,
        key: key.clone(),
        scope: scope.clone(),
        layer,
        old: raw_old,
        new: value,
        origin,
        source: ChangeSource::Api,
    };
    Ok(Some(SetPlan {
        which,
        text,
        candidate,
        change,
        entry,
    }))
}

pub(crate) fn layer_of(which: Writable, machine: &MachineId) -> ConfigLayer {
    match which {
        Writable::Shared => ConfigLayer::Shared,
        Writable::Machine => ConfigLayer::Machine(machine.clone()),
    }
}

/// Wynik udanego przeładowania.
pub(crate) struct ReloadPlan {
    pub changes: Vec<ConfigChange>,
    pub history: Vec<HistoryEntry>,
}

/// Przeładowanie: parsowanie → reguła `kernel.*` → schematy → zmiany. Błąd zostawia model,
/// a zmienione pliki oznacza jako niepoprawne (zapis do nich wstrzymany do naprawy).
pub(crate) fn apply_reload(
    st: &mut State,
    ctx: &Ctx<'_>,
    shared: Result<LayerData, ReloadError>,
    machine: Result<LayerData, ReloadError>,
) -> Result<ReloadPlan, ReloadError> {
    let shared = shared.inspect_err(|_| st.broken_shared = true)?;
    let machine = machine.inspect_err(|_| st.broken_machine = true)?;
    let candidate = Model {
        defaults: st.model.defaults.clone(),
        shared,
        machine,
    };
    let mut raw = Vec::new();
    for which in [Writable::Shared, Writable::Machine] {
        let (old, new) = (st.model.layer(which), candidate.layer(which));
        let mut scopes = old.scopes();
        scopes.extend(
            new.scopes()
                .into_iter()
                .filter(|s| !old.scopes().contains(s)),
        );
        for scope in scopes {
            for (key, o, n) in raw_diff(old.flat(&scope), new.flat(&scope)) {
                raw.push((which, scope.clone(), key, o, n));
            }
        }
    }
    let fail = |st: &mut State, err: ReloadError| {
        for (which, ..) in &raw {
            st.set_broken(*which, true);
        }
        Err(err)
    };
    if let Some((_, _, key, ..)) = raw.iter().find(|r| r.2.is_kernel_policy()) {
        let err = ReloadError::KernelPolicy(key.clone());
        return fail(st, err);
    }
    for entry in st.schemas.entries() {
        for scope in candidate.scopes() {
            if let Err(reason) = entry.validate(&candidate.subtree(&entry.prefix, &scope)) {
                let prefix = entry.prefix.clone();
                return fail(st, ReloadError::Schema { prefix, reason });
            }
        }
    }
    let mut changes: Vec<ConfigChange> = Vec::new();
    let mut history = Vec::new();
    for (which, scope, key, old_raw, new_raw) in raw {
        let layer = layer_of(which, ctx.machine);
        let old = st.model.resolve(&key, &scope).cloned();
        let new = candidate.resolve(&key, &scope).cloned();
        let seen = changes.iter().any(|c| c.key == key && c.scope == scope);
        if old != new && !seen {
            changes.push(ConfigChange {
                key: key.clone(),
                scope: scope.clone(),
                layer: layer.clone(),
                old,
                new,
                origin: Origin::User,
            });
        }
        history.push(HistoryEntry {
            ts: ctx.now,
            key,
            scope,
            layer,
            old: old_raw,
            new: new_raw,
            origin: Origin::User,
            source: ChangeSource::File,
        });
    }
    st.model = candidate;
    st.broken_shared = false;
    st.broken_machine = false;
    Ok(ReloadPlan { changes, history })
}
