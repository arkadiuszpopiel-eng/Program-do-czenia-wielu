//! Uruchomienie przebiegu `agent-runtime` na dostawcy wybranym przez Router i odczyt jego
//! dziennika bez luk: subskrypcja + zaległe zdarzenia (`events`) scalone po numerze kolejnym,
//! po przepełnieniu kanału — ponowny odczyt dziennika. Checkpointy w pamięci procesu (treść
//! przebiegu nie trafia na dysk poza szyfrowaną bazą sesji).

use std::sync::Arc;

use agent_runtime_contract::{
    AgentRuntime, MemCheckpointStore, RunError, RunEvent, RunEventEnvelope, RunId, RunSpec, Steer,
};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use core_bus_contract::EventBus;
use providers_contract::ModelProvider;
use tokio::sync::broadcast::{self, error::RecvError};

use crate::toolset::AgentTools;

/// Uruchomiony przebieg.
pub struct RunHandle {
    runtime: Arc<Runtime>,
    run: RunId,
}

impl std::fmt::Debug for RunHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunHandle").field("run", &self.run).finish()
    }
}

impl RunHandle {
    /// Startuje przebieg (pętla w tle).
    pub async fn start(
        provider: Arc<dyn ModelProvider>,
        tools: &AgentTools,
        bus: Option<Arc<dyn EventBus>>,
        spec: RunSpec,
    ) -> Result<(Self, RunFeed), RunError> {
        let runtime = Arc::new(Runtime::new(RuntimeDeps {
            provider,
            tools: tools.all(),
            checkpoints: Arc::new(MemCheckpointStore::default()),
            bus,
            config: RuntimeConfig::default(),
        }));
        let run = runtime.start(spec).await?;
        let rx = runtime.subscribe(&run)?;
        let mut feed = RunFeed {
            runtime: runtime.clone(),
            run: run.clone(),
            rx,
            seen: 0,
            backlog: Vec::new(),
            done: false,
        };
        // Zdarzenia sprzed subskrypcji (pętla startuje natychmiast w tle).
        feed.refill();
        Ok((Self { runtime, run }, feed))
    }

    /// Start v1 (`start_with`): runtime z zasobami wyłącznymi i poziomami autonomii, opcje z obsadą
    /// (delegacja, Krytyczka); dziennik całej rodziny przebiegów (podprzebiegi w Replay).
    pub async fn launch(
        launch: &crate::Launch,
        provider: Arc<dyn ModelProvider>,
        tools: &AgentTools,
        bus: Option<Arc<dyn EventBus>>,
        spec: RunSpec,
        options: agent_runtime_contract::RunOptions,
    ) -> Result<(Self, crate::RunFamily), RunError> {
        let (runtime, store) = launch.runtime(provider, tools.all(), bus);
        let run = crate::Launch::start(&runtime, spec, options).await?;
        let family = crate::RunFamily::attach(runtime.clone(), store, run.clone())?;
        Ok((Self { runtime, run }, family))
    }

    /// Identyfikator przebiegu.
    pub fn id(&self) -> &RunId {
        &self.run
    }

    /// Wiadomość właściciela w trakcie (uwzględniona w następnym kroku).
    pub fn steer(&self, message: String) -> Result<(), RunError> {
        self.runtime.steer(&self.run, Steer::Message(message))
    }

    /// Anulowanie (narzędzia, model, czekanie na zgodę dostają sygnał).
    pub fn cancel(&self) {
        // Przebieg nieznany = już zakończony — nie ma czego anulować.
        let _ = self.runtime.cancel(&self.run);
    }
}

/// Dziennik przebiegu na żywo (bez luk, bez powtórzeń, do `Finished`).
pub struct RunFeed {
    runtime: Arc<Runtime>,
    run: RunId,
    rx: broadcast::Receiver<RunEventEnvelope>,
    seen: u64,
    backlog: Vec<RunEventEnvelope>,
    done: bool,
}

impl RunFeed {
    /// Dziennik istniejącego przebiegu (np. zadania schedulera wykonywanego przez
    /// `RuntimeExecutor`): zaległe zdarzenia od początku, potem na żywo.
    pub fn attach(runtime: Arc<Runtime>, run: RunId) -> Result<Self, RunError> {
        let rx = runtime.subscribe(&run)?;
        let mut feed = Self {
            runtime,
            run,
            rx,
            seen: 0,
            backlog: Vec::new(),
            done: false,
        };
        feed.refill();
        Ok(feed)
    }

    fn refill(&mut self) {
        let mut log = self.runtime.events(&self.run).unwrap_or_default();
        log.retain(|e| e.seq > self.seen);
        log.reverse();
        self.backlog = log;
    }

    fn take(&mut self, env: RunEventEnvelope) -> Option<RunEventEnvelope> {
        if env.seq <= self.seen {
            return None;
        }
        if env.seq > self.seen + 1 {
            // Luka (zdarzenia przed subskrypcją) — najpierw zaległe z dziennika.
            self.refill();
            return self.backlog.pop().inspect(|e| self.mark(e));
        }
        self.mark(&env);
        Some(env)
    }

    fn mark(&mut self, env: &RunEventEnvelope) {
        self.seen = env.seq;
        if matches!(env.event, RunEvent::Finished { .. }) {
            self.done = true;
        }
    }

    /// Następne zdarzenie; `None` po `Finished` albo gdy przebieg zniknął.
    pub async fn next(&mut self) -> Option<RunEventEnvelope> {
        loop {
            if let Some(env) = self.backlog.pop() {
                if env.seq > self.seen {
                    self.mark(&env);
                    return Some(env);
                }
                continue;
            }
            if self.done {
                return None;
            }
            match self.rx.recv().await {
                Ok(env) => {
                    if let Some(env) = self.take(env) {
                        return Some(env);
                    }
                }
                Err(RecvError::Lagged(_)) => self.refill(),
                Err(RecvError::Closed) => {
                    self.refill();
                    if self.backlog.is_empty() {
                        return None;
                    }
                }
            }
        }
    }
}
