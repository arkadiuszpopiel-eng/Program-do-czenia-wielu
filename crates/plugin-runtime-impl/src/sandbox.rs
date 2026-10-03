//! Jedno wywołanie w piaskownicy: **nowy `Store` i nowa instancja na każde wywołanie** (brak
//! współdzielonej pamięci między wtyczkami i wywołaniami), limiter pamięci/tabel/instancji,
//! paliwo, przerwanie epokowe (czas Wasm bez czasu operacji hosta, anulowanie), klasyfikacja
//! pułapek do [`ExecError`] — błąd wtyczki nigdy nie jest paniką hosta.

use std::sync::Arc;
use std::time::{Duration, Instant};

use core_bus_contract::Event;
use plugin_runtime_contract::{
    EXPORT_INVOKE, ExecError, InvocationStats, PluginHost, PluginLimits, sanitize,
};
use safety_broker_contract::Capability;
use tokio_util::sync::CancellationToken;
use tools_common_contract::{BrokerGate, ToolCtx};
use wasmtime::component::InstancePre;
use wasmtime::{ResourceLimiter, Store, Trap, UpdateDeadline};

/// Najwięcej instancji rdzeniowych w komponencie (moduł, pamięć, shim, fixup…).
const MAX_INSTANCES: usize = 16;
/// Najwięcej tabel.
const MAX_TABLES: usize = 8;
/// Najwięcej pamięci.
const MAX_MEMORIES: usize = 2;
/// Najwięcej elementów tabeli.
const MAX_TABLE_ELEMENTS: usize = 10_000;

/// Powód przerwania ustawiany przez hosta (rozpoznawany niezależnie od komunikatu pułapki).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SandboxStop {
    /// Anulowanie.
    #[error("anulowano")]
    Cancelled,
    /// Czas Wasm.
    #[error("limit czasu")]
    Timeout,
    /// Limit operacji hosta.
    #[error("limit operacji hosta")]
    HostCalls,
}

/// Limiter zasobów (zapamiętuje odmowę wzrostu pamięci).
pub(crate) struct SandboxLimiter {
    memory_bytes: usize,
    pub(crate) memory_denied: bool,
}

impl ResourceLimiter for SandboxLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.memory_bytes || maximum.is_some_and(|m| desired > m) {
            self.memory_denied = true;
            return Err(wasmtime::Error::msg("limit pamięci wtyczki"));
        }
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= MAX_TABLE_ELEMENTS)
    }

    fn instances(&self) -> usize {
        MAX_INSTANCES
    }

    fn tables(&self) -> usize {
        MAX_TABLES
    }

    fn memories(&self) -> usize {
        MAX_MEMORIES
    }
}

/// Kontekst operacji hosta (agentka wywołująca, Broker, port wykonawczy).
pub(crate) struct HostCtx {
    pub gate: BrokerGate,
    pub host: Arc<dyn PluginHost>,
    pub tool_ctx: ToolCtx,
    pub declared: Vec<Capability>,
    pub tool_id: String,
    pub handle: tokio::runtime::Handle,
    pub token_ttl_ms: u64,
}

/// Dane `Store` jednego wywołania.
pub(crate) struct StoreData {
    pub limiter: SandboxLimiter,
    pub limits: PluginLimits,
    pub host: Option<HostCtx>,
    pub started: Instant,
    pub host_time: Duration,
    pub cancel: CancellationToken,
    pub stop: Option<SandboxStop>,
    pub stats: InvocationStats,
    pub events: Vec<Event>,
}

impl StoreData {
    /// Dane dla wywołania.
    pub(crate) fn new(limits: PluginLimits, host: Option<HostCtx>, stats: InvocationStats) -> Self {
        let cancel = host
            .as_ref()
            .map(|h| h.tool_ctx.cancel.clone())
            .unwrap_or_default();
        Self {
            limiter: SandboxLimiter {
                memory_bytes: limits.memory_bytes(),
                memory_denied: false,
            },
            limits,
            host,
            started: Instant::now(),
            host_time: Duration::ZERO,
            cancel,
            stop: None,
            stats,
            events: Vec::new(),
        }
    }

    /// Czas wykonania Wasm (bez operacji hosta).
    pub(crate) fn wasm_elapsed(&self) -> Duration {
        self.started.elapsed().saturating_sub(self.host_time)
    }
}

/// Wynik wywołania: tekst wyniku albo błąd wtyczki, statystyki, zdarzenia z operacji hosta.
pub(crate) struct RunResult {
    pub output: Result<Result<String, String>, ExecError>,
    pub stats: InvocationStats,
    pub events: Vec<Event>,
}

fn classify(err: &wasmtime::Error, data: &StoreData) -> ExecError {
    if data.limiter.memory_denied {
        return ExecError::MemoryLimit;
    }
    match data
        .stop
        .or_else(|| err.downcast_ref::<SandboxStop>().copied())
    {
        Some(SandboxStop::Cancelled) => return ExecError::Cancelled,
        Some(SandboxStop::Timeout) => return ExecError::Timeout,
        Some(SandboxStop::HostCalls) => return ExecError::HostCallLimit,
        None => {}
    }
    match err.downcast_ref::<Trap>() {
        Some(Trap::OutOfFuel) => ExecError::OutOfFuel,
        Some(Trap::StackOverflow) => ExecError::StackOverflow,
        Some(Trap::Interrupt) => ExecError::Timeout,
        Some(trap) => ExecError::Trap(sanitize(&trap.to_string())),
        None => ExecError::Trap(sanitize(&err.root_cause().to_string())),
    }
}

/// Wywołanie `invoke(tool, input)` w świeżej instancji (wątek blokujący).
pub(crate) fn run(
    pre: &InstancePre<StoreData>,
    data: StoreData,
    tool: &str,
    input: &str,
) -> RunResult {
    let fuel = data.limits.fuel_per_call;
    let mut store = Store::new(pre.engine(), data);
    store.limiter(|d| &mut d.limiter);
    store.epoch_deadline_callback(|mut ctx| {
        let d = ctx.data_mut();
        let stop = if d.cancel.is_cancelled() {
            Some(SandboxStop::Cancelled)
        } else if d.wasm_elapsed() > Duration::from_millis(d.limits.wall_ms) {
            Some(SandboxStop::Timeout)
        } else {
            None
        };
        match stop {
            Some(s) => {
                d.stop = Some(s);
                Err(wasmtime::Error::new(s))
            }
            None => Ok(UpdateDeadline::Continue(1)),
        }
    });
    store.set_epoch_deadline(1);
    let output = store
        .set_fuel(fuel)
        .and_then(|()| call(pre, &mut store, tool, input));
    let output = output.map_err(|e| classify(&e, store.data()));
    let used = fuel.saturating_sub(store.get_fuel().unwrap_or(0));
    let data = store.into_data();
    let mut stats = data.stats;
    stats.fuel_used = used;
    stats.wasm_ms = u64::try_from(
        data.started
            .elapsed()
            .saturating_sub(data.host_time)
            .as_millis(),
    )
    .unwrap_or(u64::MAX);
    stats.result = match &output {
        Ok(Ok(_)) => "ok".into(),
        Ok(Err(_)) => ExecError::PluginFailed(String::new()).kind().into(),
        Err(e) => e.kind().into(),
    };
    RunResult {
        output,
        stats,
        events: data.events,
    }
}

fn call(
    pre: &InstancePre<StoreData>,
    store: &mut Store<StoreData>,
    tool: &str,
    input: &str,
) -> wasmtime::Result<Result<String, String>> {
    let instance = pre.instantiate(&mut *store)?;
    let func = instance
        .get_typed_func::<(&str, &str), (Result<String, String>,)>(&mut *store, EXPORT_INVOKE)?;
    let (out,) = func.call(&mut *store, (tool, input))?;
    func.post_return(&mut *store)?;
    Ok(out)
}
