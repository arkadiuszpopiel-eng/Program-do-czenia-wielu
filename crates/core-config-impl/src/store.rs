//! `FileConfigStore` — konfiguracja warstwowa na plikach TOML.
//!
//! Układ katalogu: `shared.toml` (warstwa wspólna), `machine/<id>.toml` (nakładka bieżącej
//! maszyny), `history.ndjson` (historia append-only). Zapis atomowy: plik tymczasowy + fsync +
//! `rename`. Operacje są serializowane jednym zamkiem (zapisy są rzadkie).

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use core_bus_contract::{Event, EventBus, EventKind, Level};
use core_config_contract::{
    ConfigChange, ConfigError, ConfigKey, ConfigLayer, ConfigStore, ConfigValue, ConfigWatch,
    MachineId, Origin, Scope, authorize,
};
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::apply::{Ctx, ReloadError, State, apply_reload, check_value, plan_set, writable};
use crate::history::{self, HistoryEntry};
use crate::layer::LayerData;
use crate::model::Writable;

/// Zdarzenie zmiany wartości wynikowej (ładunek = `ConfigChange`).
pub const EVENT_CHANGED: &str = "config.changed";
/// Zdarzenie udanego przeładowania plików (`{changes}`).
pub const EVENT_RELOADED: &str = "config.reloaded";
/// Zdarzenie odrzuconego przeładowania (`{reason}`); wartości bez zmian.
pub const EVENT_INVALID: &str = "config.invalid";
/// Zdarzenie odrzuconej zmiany polityki Jądra (`{key, origin}`).
pub const EVENT_KERNEL_POLICY_REJECTED: &str = "config.kernel_policy_rejected";

/// Nazwa pliku historii.
pub const HISTORY_FILE: &str = "history.ndjson";

/// Źródło czasu (znaczniki historii).
pub trait Clock: Send + Sync {
    /// Bieżący czas.
    fn now(&self) -> DateTime<Utc>;
}

impl<F: Fn() -> DateTime<Utc> + Send + Sync> Clock for F {
    fn now(&self) -> DateTime<Utc> {
        self()
    }
}

/// Ustawienia magazynu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigOptions {
    /// Katalog konfiguracji (np. `%APPDATA%\Alfa\config`).
    pub dir: PathBuf,
    /// Bieżąca maszyna (nazwa pliku nakładki).
    pub machine: MachineId,
    /// Tryb ścisły: klucz spoza zarejestrowanych schematów → `UnknownKey`.
    pub strict_keys: bool,
}

impl ConfigOptions {
    /// Ustawienia domyślne (tryb nieścisły).
    pub fn new(dir: impl Into<PathBuf>, machine: MachineId) -> Self {
        Self {
            dir: dir.into(),
            machine,
            strict_keys: false,
        }
    }
}

/// Magazyn konfiguracji na plikach TOML (implementuje `ConfigStore`).
pub struct FileConfigStore {
    options: ConfigOptions,
    clock: Arc<dyn Clock>,
    bus: Option<Arc<dyn EventBus>>,
    state: Mutex<State>,
    watchers: Mutex<Vec<(String, UnboundedSender<ConfigChange>)>>,
    load_problems: Vec<String>,
}

fn persist(context: &str, e: impl std::fmt::Display) -> ConfigError {
    ConfigError::Persist(format!("{context}: {e}"))
}

fn valid_machine_id(id: &MachineId) -> bool {
    !id.0.is_empty()
        && id.0.len() <= 64
        && id
            .0
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn load(path: &Path) -> Result<LayerData, ReloadError> {
    let invalid = |reason: String| ReloadError::Invalid {
        file: path.to_owned(),
        reason,
    };
    match fs::read_to_string(path) {
        Ok(text) => LayerData::parse(&text).map_err(invalid),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LayerData::default()),
        Err(e) => Err(invalid(e.to_string())),
    }
}

/// Zapis atomowy: `<plik>.tmp-<pid>` + fsync, potem `rename` na miejsce docelowe.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".tmp-{}", std::process::id()));
    let tmp = path.with_file_name(name);
    let written = File::create(&tmp).and_then(|mut f| {
        f.write_all(text.as_bytes())?;
        f.sync_all()
    });
    match written.and_then(|()| fs::rename(&tmp, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

impl FileConfigStore {
    /// Otwiera katalog i wczytuje warstwy. Niepoprawny plik nie blokuje startu: warstwa jest
    /// pusta, problem trafia do `load_problems()`, a zapis do tego pliku jest wstrzymany.
    /// Klucze `kernel.*` obecne w plikach przy starcie są przyjmowane (stan zastany).
    pub fn open(options: ConfigOptions, clock: Arc<dyn Clock>) -> Result<Self, ConfigError> {
        if !valid_machine_id(&options.machine) {
            return Err(ConfigError::Persist(format!(
                "niepoprawny identyfikator maszyny `{}` (dozwolone [A-Za-z0-9_-], ≤ 64)",
                options.machine.0
            )));
        }
        fs::create_dir_all(options.dir.join("machine")).map_err(|e| persist("katalog", e))?;
        let mut state = State::default();
        let mut load_problems = Vec::new();
        for which in [Writable::Shared, Writable::Machine] {
            let path = layer_path(&options, which);
            match load(&path) {
                Ok(data) => *state.model.layer_mut(which) = data,
                Err(e) => {
                    state.set_broken(which, true);
                    load_problems.push(e.to_string());
                }
            }
        }
        Ok(Self {
            options,
            clock,
            bus: None,
            state: Mutex::new(state),
            watchers: Mutex::new(Vec::new()),
            load_problems,
        })
    }

    /// Podpina magistralę (zdarzenia `config.*`).
    #[must_use]
    pub fn with_bus(mut self, bus: Arc<dyn EventBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Problemy z plikami wykryte przy otwarciu.
    pub fn load_problems(&self) -> &[String] {
        &self.load_problems
    }

    /// Katalog konfiguracji.
    pub fn dir(&self) -> &Path {
        &self.options.dir
    }

    /// Rejestruje JSON Schema poddrzewa `prefix.*` (z modułu); `default` → warstwa Default.
    pub fn register_schema(&self, prefix: &ConfigKey, schema: &Value) -> Result<(), ConfigError> {
        let mut st = self.lock();
        st.schemas
            .register(prefix.clone(), schema)
            .map_err(|reason| ConfigError::SchemaViolation {
                key: prefix.clone(),
                reason,
            })?;
        st.model.defaults = st.schemas.defaults();
        Ok(())
    }

    /// Historia zmian (opcjonalnie tylko jednego klucza), od najstarszej.
    pub fn history(&self, key: Option<&ConfigKey>) -> Result<Vec<HistoryEntry>, ConfigError> {
        let all = history::read(&self.options.dir.join(HISTORY_FILE))
            .map_err(|e| persist("historia", e))?;
        Ok(all
            .into_iter()
            .filter(|e| key.is_none_or(|k| e.key == *k))
            .collect())
    }

    /// Jawne przeładowanie plików (też wywoływane przez obserwatora plików). Błąd → wartości
    /// bez zmian, zdarzenie `config.invalid`. Sukces → `ConfigChange` dla zmienionych kluczy.
    pub async fn reload(&self) -> Result<Vec<ConfigChange>, ReloadError> {
        let result = {
            // Odczyt pod zamkiem: równoległy `set` nie może zostać cofnięty nieaktualnym odczytem.
            let mut st = self.lock();
            let shared = load(&layer_path(&self.options, Writable::Shared));
            let machine = load(&layer_path(&self.options, Writable::Machine));
            apply_reload(&mut st, &self.ctx(), shared, machine)
        };
        match result {
            Ok(plan) => {
                self.record_history(&plan.history);
                self.announce(&plan.changes).await;
                let payload = json!({"changes": plan.changes.len()});
                self.publish(EVENT_RELOADED, Level::Info, payload).await;
                Ok(plan.changes)
            }
            Err(e) => {
                let payload = json!({"reason": e.to_string()});
                self.publish(EVENT_INVALID, Level::Warn, payload).await;
                Err(e)
            }
        }
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx {
            machine: &self.options.machine,
            strict_keys: self.options.strict_keys,
            now: self.clock.now(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn record_history(&self, entries: &[HistoryEntry]) {
        if let Err(e) = history::append(&self.options.dir.join(HISTORY_FILE), entries) {
            tracing::warn!(error = %e, "nie zapisano historii konfiguracji");
        }
    }

    async fn publish(&self, name: &str, level: Level, payload: Value) {
        if let Some(bus) = &self.bus {
            let event = Event::new(EventKind::Custom(name.to_owned()), level, payload);
            if let Err(e) = bus.publish(event).await {
                tracing::warn!(error = %e, zdarzenie = name, "nie opublikowano zdarzenia");
            }
        }
    }

    /// Powiadamia obserwatorów (prefiks) i magistralę o zmianach wartości wynikowych.
    async fn announce(&self, changes: &[ConfigChange]) {
        {
            let mut watchers = self.watchers.lock().unwrap_or_else(|p| p.into_inner());
            watchers.retain(|(prefix, tx)| {
                changes
                    .iter()
                    .filter(|c| c.key.has_prefix(prefix))
                    .all(|c| tx.send(c.clone()).is_ok())
                    && !tx.is_closed()
            });
        }
        for change in changes {
            let payload = serde_json::to_value(change).unwrap_or_default();
            self.publish(EVENT_CHANGED, Level::Info, payload).await;
        }
    }

    fn apply_set(
        &self,
        key: &ConfigKey,
        value: Option<ConfigValue>,
        scope: &Scope,
        layer: &ConfigLayer,
        origin: Origin,
    ) -> Result<Option<ConfigChange>, ConfigError> {
        let which = writable(layer, &self.options.machine)?;
        if let Some(v) = &value {
            check_value(key, v)?;
        }
        let mut st = self.lock();
        let Some(plan) = plan_set(&st, &self.ctx(), key, value, scope, which, origin)? else {
            return Ok(None);
        };
        write_atomic(&layer_path(&self.options, plan.which), &plan.text)
            .map_err(|e| persist("zapis pliku warstwy", e))?;
        st.model = plan.candidate;
        drop(st);
        self.record_history(std::slice::from_ref(&plan.entry));
        Ok(plan.change)
    }
}

fn layer_path(options: &ConfigOptions, which: Writable) -> PathBuf {
    match which {
        Writable::Shared => options.dir.join("shared.toml"),
        Writable::Machine => options
            .dir
            .join("machine")
            .join(format!("{}.toml", options.machine.0)),
    }
}

#[async_trait]
impl ConfigStore for FileConfigStore {
    async fn get(
        &self,
        key: &ConfigKey,
        scope: &Scope,
    ) -> Result<Option<ConfigValue>, ConfigError> {
        Ok(self.lock().model.resolve(key, scope).cloned())
    }

    async fn set(
        &self,
        key: &ConfigKey,
        value: Option<ConfigValue>,
        scope: &Scope,
        layer: &ConfigLayer,
        origin: Origin,
    ) -> Result<(), ConfigError> {
        if let Err(e) = authorize(key, &origin) {
            let payload = json!({"key": key.as_str(), "origin": origin});
            self.publish(EVENT_KERNEL_POLICY_REJECTED, Level::Warn, payload)
                .await;
            return Err(e);
        }
        if let Some(change) = self.apply_set(key, value, scope, layer, origin)? {
            self.announce(std::slice::from_ref(&change)).await;
        }
        Ok(())
    }

    fn watch(&self, prefix: &str) -> ConfigWatch {
        let (tx, rx) = unbounded_channel();
        self.watchers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((prefix.to_owned(), tx));
        Box::pin(UnboundedReceiverStream::new(rx))
    }
}
