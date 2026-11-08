//! Silnik piaskownicy: konfiguracja wasmtime (paliwo, epoch, stos, wyłączone propozycje
//! Wasm), wątek zegara epok, kompilacja komponentu z kontrolą importów (wyłącznie
//! `alfa:plugin/host@0.1.0` z funkcją `call`) i eksportów (wyłącznie `invoke`), wstępne
//! łączenie z hostem (`InstancePre`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use plugin_runtime_contract::{
    EXPORT_INVOKE, HOST_CALL, HOST_INTERFACE, LoadError, MAX_WASM_BYTES, is_component, sanitize,
};
use wasmtime::component::types::{ComponentFunc, ComponentItem};
use wasmtime::component::{Component, InstancePre, Linker, Type};
use wasmtime::{Config, Engine, OptLevel};

use crate::hostcall::host_call;
use crate::sandbox::StoreData;

/// Stos Wasm (B) — głęboka rekurencja kończy się pułapką `StackOverflow`, nie awarią hosta.
pub const MAX_WASM_STACK: usize = 512 * 1024;

/// Zegar epok: wątek zwiększa epokę silnika co `tick`, póki istnieje.
struct Ticker {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Silnik + linker z funkcją hosta.
pub(crate) struct SandboxEngine {
    engine: Engine,
    linker: Linker<StoreData>,
    tick: Duration,
    _ticker: Ticker,
}

fn config() -> Config {
    let mut c = Config::new();
    c.consume_fuel(true)
        .epoch_interruption(true)
        .max_wasm_stack(MAX_WASM_STACK)
        .wasm_component_model(true)
        .wasm_relaxed_simd(false)
        .wasm_memory64(false)
        .wasm_multi_memory(false)
        .wasm_custom_page_sizes(false)
        .wasm_wide_arithmetic(false)
        .wasm_stack_switching(false)
        .cranelift_opt_level(OptLevel::Speed);
    c
}

impl SandboxEngine {
    /// Silnik z zegarem epok o podanym kroku.
    pub(crate) fn new(tick: Duration) -> Result<Self, String> {
        let engine = Engine::new(&config()).map_err(|e| e.to_string())?;
        let mut linker = Linker::new(&engine);
        linker
            .instance(HOST_INTERFACE)
            .and_then(|mut i| i.func_wrap(HOST_CALL, host_call))
            .map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, eng) = (stop.clone(), engine.clone());
        let handle = std::thread::Builder::new()
            .name("alfa-plugin-epoch".into())
            .spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(tick);
                    eng.increment_epoch();
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            engine,
            linker,
            tick,
            _ticker: Ticker {
                stop,
                handle: Some(handle),
            },
        })
    }

    /// Krok zegara epok.
    pub(crate) fn tick(&self) -> Duration {
        self.tick
    }

    /// Kompilacja i kontrola komponentu (bajty już zweryfikowane hashem przez wywołującego).
    pub(crate) fn compile(&self, bytes: &[u8]) -> Result<InstancePre<StoreData>, LoadError> {
        if bytes.len() > MAX_WASM_BYTES {
            return Err(LoadError::TooLarge(bytes.len()));
        }
        if !is_component(bytes) {
            return Err(LoadError::NotComponent);
        }
        let component = Component::from_binary(&self.engine, bytes)
            .map_err(|e| LoadError::Invalid(sanitize(&e.root_cause().to_string())))?;
        check_imports(&self.engine, &component)?;
        check_exports(&self.engine, &component)?;
        self.linker
            .instantiate_pre(&component)
            .map_err(|e| LoadError::ImportType(sanitize(&e.root_cause().to_string())))
    }
}

/// Czy funkcja ma sygnaturę `func(string, string) -> result<string, string>`.
fn is_string_pair_to_result(f: &ComponentFunc) -> bool {
    let params_ok = f.params().len() == 2 && f.params().all(|(_, t)| t == Type::String);
    let mut results = f.results();
    let result_ok = results.len() == 1
        && matches!(results.next(), Some(Type::Result(r))
            if r.ok() == Some(Type::String) && r.err() == Some(Type::String));
    params_ok && result_ok
}

fn check_imports(engine: &Engine, component: &Component) -> Result<(), LoadError> {
    for (name, item) in component.component_type().imports(engine) {
        if name != HOST_INTERFACE {
            return Err(LoadError::ForbiddenImport(sanitize(name)));
        }
        let ComponentItem::ComponentInstance(instance) = item else {
            return Err(LoadError::ImportType(format!(
                "`{HOST_INTERFACE}` nie jest instancją"
            )));
        };
        for (export, ty) in instance.exports(engine) {
            let ok = export == HOST_CALL
                && matches!(&ty, ComponentItem::ComponentFunc(f) if is_string_pair_to_result(f));
            if !ok {
                return Err(LoadError::ImportType(format!(
                    "`{HOST_INTERFACE}` może zawierać wyłącznie `{HOST_CALL}: func(string, string) -> result<string, string>` (jest `{}`)",
                    sanitize(export)
                )));
            }
        }
    }
    Ok(())
}

fn check_exports(engine: &Engine, component: &Component) -> Result<(), LoadError> {
    let mut found = false;
    for (name, item) in component.component_type().exports(engine) {
        if name != EXPORT_INVOKE {
            return Err(LoadError::ExportType(format!(
                "niedozwolony eksport `{}` (wtyczka eksportuje wyłącznie `{EXPORT_INVOKE}`)",
                sanitize(name)
            )));
        }
        match item {
            ComponentItem::ComponentFunc(f) if is_string_pair_to_result(&f) => found = true,
            _ => {
                return Err(LoadError::ExportType(format!(
                    "`{EXPORT_INVOKE}` musi mieć typ func(string, string) -> result<string, string>"
                )));
            }
        }
    }
    if found {
        Ok(())
    } else {
        Err(LoadError::MissingExport(EXPORT_INVOKE.to_owned()))
    }
}
