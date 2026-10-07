//! Atrapa `plugin-runtime` (docs/modules/plugin-runtime/SPEC.md, „Fake”): rdzeń
//! [`PluginLibrary`] z kontraktu, moduły w pamięci (wystarczy nagłówek komponentu i zgodny
//! hash), **zachowania zamiast Wasm** (funkcje Rust per hash modułu), wirtualny zegar,
//! nagrane zdarzenia i wywołania. Ta sama obróbka wejścia/wyjścia co w `-impl`
//! (`check_input`, `parse_output`, wynik niezaufany). Do testów UI, `app-*`, `improver` —
//! wyłącznie jako dev-dependency.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::Event;
use plugin_runtime_contract::{
    ExecError, InvocationStats, PluginApproval, PluginError, PluginId, PluginLibrary,
    PluginManifest, PluginRecord, PluginSource, PluginToolDecl, Plugins, UNTRUSTED_SOURCE,
    check_input, check_wasm, ok_outcome, parse_output, validate_manifest,
};
use semver::Version;
use serde_json::Value;
use tools_common_contract::{Tool, ToolCtx, ToolManifest, ToolOutcome};

/// Zachowanie „wtyczki”: (narzędzie, argumenty) → dane wyniku albo błąd wykonania.
pub type Behavior = Arc<dyn Fn(&str, &Value) -> Result<Value, ExecError> + Send + Sync>;

/// Wywołanie zarejestrowane przez atrapę.
#[derive(Debug, Clone, PartialEq)]
pub struct FakeCall {
    /// Wtyczka.
    pub plugin: String,
    /// Narzędzie (nazwa w wtyczce).
    pub tool: String,
    /// Argumenty.
    pub input: Value,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Licznik słów jak wtyczka przykładowa (`samples::word_count_tool`).
pub fn word_count_behavior() -> Behavior {
    Arc::new(|_tool, input| {
        let text = input
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let words = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .count();
        Ok(serde_json::json!({ "words": words }))
    })
}

struct State {
    lib: Mutex<PluginLibrary>,
    wasm: Mutex<BTreeMap<String, Vec<u8>>>,
    behaviors: Mutex<BTreeMap<String, Behavior>>,
    default_behavior: Mutex<Option<Behavior>>,
    events: Mutex<Vec<Event>>,
    calls: Mutex<Vec<FakeCall>>,
    clock: AtomicU64,
}

/// Atrapa środowiska wtyczek.
#[derive(Clone)]
pub struct FakePlugins {
    state: Arc<State>,
}

impl Default for FakePlugins {
    fn default() -> Self {
        Self::new()
    }
}

impl FakePlugins {
    /// Pusta biblioteka (czas 0, zachowanie domyślne: licznik słów).
    pub fn new() -> Self {
        Self {
            state: Arc::new(State {
                lib: Mutex::new(PluginLibrary::default()),
                wasm: Mutex::new(BTreeMap::new()),
                behaviors: Mutex::new(BTreeMap::new()),
                default_behavior: Mutex::new(Some(word_count_behavior())),
                events: Mutex::new(Vec::new()),
                calls: Mutex::new(Vec::new()),
                clock: AtomicU64::new(0),
            }),
        }
    }

    /// Zachowanie modułu o danym hashu.
    pub fn behave(&self, wasm_sha256: &str, behavior: Behavior) {
        lock(&self.state.behaviors).insert(wasm_sha256.to_owned(), behavior);
    }

    /// Zachowanie domyślne (`None` = moduł bez zachowania kończy się pułapką).
    pub fn default_behavior(&self, behavior: Option<Behavior>) {
        *lock(&self.state.default_behavior) = behavior;
    }

    /// Przesuwa wirtualny zegar.
    pub fn advance(&self, ms: u64) {
        self.state.clock.fetch_add(ms, Ordering::SeqCst);
    }

    /// Nagrane zdarzenia.
    pub fn events(&self) -> Vec<Event> {
        lock(&self.state.events).clone()
    }

    /// Nagrane wywołania narzędzi.
    pub fn calls(&self) -> Vec<FakeCall> {
        lock(&self.state.calls).clone()
    }

    fn now(&self) -> u64 {
        self.state.clock.load(Ordering::SeqCst)
    }

    fn apply<T>(
        &self,
        f: impl FnOnce(&mut PluginLibrary, u64) -> Result<(T, Vec<Event>), PluginError>,
    ) -> Result<T, PluginError> {
        let now = self.now();
        let mut lib = lock(&self.state.lib);
        let mut next = lib.clone();
        let (value, events) = f(&mut next, now)?;
        *lib = next;
        drop(lib);
        lock(&self.state.events).extend(events);
        Ok(value)
    }
}

impl State {
    fn run(&self, id: &PluginId, decl: &PluginToolDecl, args: Value) -> ToolOutcome {
        let action = format!("{} (wtyczka {id})", decl.title);
        let Some(record) = lock(&self.lib).installed(id).cloned() else {
            return ExecError::Unavailable(id.to_string()).to_outcome(&action);
        };
        let input = args.to_string();
        if let Err(e) = check_input(&input, &record.manifest.limits) {
            return e.to_outcome(&action);
        }
        lock(&self.calls).push(FakeCall {
            plugin: id.to_string(),
            tool: decl.name.clone(),
            input: args.clone(),
        });
        let behavior = lock(&self.behaviors)
            .get(&record.manifest.wasm_sha256)
            .cloned()
            .or_else(|| lock(&self.default_behavior).clone());
        let mut stats = InvocationStats::new(&record.manifest, &decl.name);
        let result = match behavior {
            Some(b) => b(&decl.name, &args)
                .and_then(|v| parse_output(&v.to_string(), decl, &record.manifest.limits)),
            None => Err(ExecError::Trap("atrapa: brak zachowania modułu".into())),
        };
        stats.result = match &result {
            Ok(_) => "ok".into(),
            Err(e) => e.kind().into(),
        };
        lock(&self.events).push(stats.event(result.as_ref().err()));
        match result {
            Ok(v) => ok_outcome(v),
            Err(e @ ExecError::PluginFailed(_)) => {
                e.to_outcome(&action).untrusted(UNTRUSTED_SOURCE)
            }
            Err(e) => e.to_outcome(&action),
        }
    }
}

struct FakeTool {
    state: Arc<State>,
    plugin: PluginId,
    decl: PluginToolDecl,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for FakeTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        self.state.run(&self.plugin, &self.decl, args)
    }
}

#[async_trait]
impl Plugins for FakePlugins {
    fn list(&self) -> Vec<PluginRecord> {
        lock(&self.state.lib).records().to_vec()
    }

    fn installed(&self, id: &PluginId) -> Option<PluginRecord> {
        lock(&self.state.lib).installed(id).cloned()
    }

    fn tool_catalog(&self) -> Vec<ToolManifest> {
        lock(&self.state.lib).tool_catalog()
    }

    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        let lib = lock(&self.state.lib);
        lib.active()
            .flat_map(|r| {
                r.manifest.tools.iter().map(|d| {
                    Arc::new(FakeTool {
                        state: self.state.clone(),
                        plugin: r.manifest.id.clone(),
                        decl: d.clone(),
                        manifest: r.manifest.tool_manifest(d),
                    }) as Arc<dyn Tool>
                })
            })
            .collect()
    }

    async fn propose(
        &self,
        manifest: PluginManifest,
        wasm: Vec<u8>,
        source: PluginSource,
    ) -> Result<PluginRecord, PluginError> {
        validate_manifest(&manifest)?;
        check_wasm(&manifest, &wasm)?;
        let sha = manifest.wasm_sha256.clone();
        let record = self.apply(|l, now| l.propose(manifest, source, now))?;
        lock(&self.state.wasm).insert(sha, wasm);
        Ok(record)
    }

    async fn approve(
        &self,
        id: &PluginId,
        version: &Version,
        approval: PluginApproval,
    ) -> Result<PluginRecord, PluginError> {
        self.apply(|l, now| l.approve(id, version, approval, now))
    }

    async fn reject(&self, id: &PluginId, version: &Version) -> Result<PluginRecord, PluginError> {
        self.apply(|l, now| l.reject(id, version, now))
    }

    async fn disable(&self, id: &PluginId) -> Result<PluginRecord, PluginError> {
        self.apply(|l, now| l.disable(id, now))
    }

    async fn enable(
        &self,
        id: &PluginId,
        approval: PluginApproval,
    ) -> Result<PluginRecord, PluginError> {
        self.apply(|l, now| l.enable(id, approval, now))
    }

    async fn remove(&self, id: &PluginId) -> Result<Vec<PluginRecord>, PluginError> {
        let gone = self.apply(|l, _| l.remove(id))?;
        let lib = lock(&self.state.lib).clone();
        let mut wasm = lock(&self.state.wasm);
        for r in &gone {
            if !lib.uses_wasm(&r.manifest.wasm_sha256) {
                wasm.remove(&r.manifest.wasm_sha256);
            }
        }
        Ok(gone)
    }
}
