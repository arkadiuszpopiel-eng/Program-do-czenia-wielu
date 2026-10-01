//! Pętla przebiegu: punkt atomowy (sterowanie, pauza, anulowanie, budżety, checkpoint) →
//! tura modelu → narzędzia → … → weryfikacja → wynik. Każdy krok jest zdarzeniem.

use std::sync::Arc;

use agent_runtime_contract::{
    BudgetKind, Checkpoint, CheckpointStore, RunEvent, RunOutcome, Steer,
};
use providers_contract::{ContentBlock, Message, ModelProvider, Role};
use tools_common_contract::Tool;

use crate::RuntimeConfig;
use crate::flow::append_capped;
use crate::handle::RunHandle;
use crate::prompt::{goal_message, skipped};
use crate::registry::ToolRegistry;

/// Zależności pętli.
pub(crate) struct Shared {
    pub(crate) provider: Arc<dyn ModelProvider>,
    pub(crate) tools: Vec<Arc<dyn Tool>>,
    pub(crate) store: Arc<dyn CheckpointStore>,
    pub(crate) config: RuntimeConfig,
}

/// Co dalej po turze.
pub(crate) enum Next {
    Continue,
    Finish(RunOutcome),
}

pub(crate) struct Engine {
    pub(crate) shared: Arc<Shared>,
    pub(crate) handle: Arc<RunHandle>,
    pub(crate) cp: Checkpoint,
    pub(crate) registry: ToolRegistry,
    started: tokio::time::Instant,
    base_elapsed: u64,
}

impl Engine {
    pub(crate) fn new(
        shared: Arc<Shared>,
        handle: Arc<RunHandle>,
        cp: Checkpoint,
        registry: ToolRegistry,
    ) -> Self {
        let base_elapsed = cp.usage.elapsed_ms;
        Self {
            shared,
            handle,
            cp,
            registry,
            started: tokio::time::Instant::now(),
            base_elapsed,
        }
    }

    pub(crate) fn elapsed_ms(&self) -> u64 {
        self.base_elapsed
            .saturating_add(u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX))
    }

    pub(crate) fn push(&mut self, role: Role, blocks: Vec<ContentBlock>) {
        self.cp.messages.push(Message::new(role, blocks));
    }

    pub(crate) fn trust(&mut self, text: &str) {
        let cap = self.shared.config.provenance_cap;
        append_capped(&mut self.cp.trusted_text, text, cap);
    }

    /// Zapis checkpointu (błąd magazynu nie zatrzymuje pracy, ale jest zdarzeniem).
    pub(crate) async fn checkpoint(&mut self) {
        self.cp.usage.elapsed_ms = self.elapsed_ms();
        self.cp.seq += 1;
        if self.shared.store.save(&self.cp).is_ok() {
            let (seq, step) = (self.cp.seq, self.cp.usage.steps);
            self.handle.emit(RunEvent::Checkpoint { seq, step }).await;
        }
    }

    pub(crate) fn exceeded(&self) -> Option<BudgetKind> {
        let b = &self.cp.spec.budget;
        let u = &self.cp.usage;
        if u.steps >= b.max_steps {
            return Some(BudgetKind::Steps);
        }
        if u.tokens() >= b.max_tokens {
            return Some(BudgetKind::Tokens);
        }
        if self.elapsed_ms() >= b.max_wall_ms {
            return Some(BudgetKind::Wall);
        }
        match b.max_cost_micro_usd {
            Some(max) if u.cost_nano_usd / 1000 >= max => Some(BudgetKind::Cost),
            _ => None,
        }
    }

    /// Start świeży albo po restarcie: wiadomość celu, wyniki dla przerwanych wywołań.
    async fn prepare(&mut self) {
        if self.cp.messages.is_empty() {
            let spec = self.cp.spec.clone();
            self.cp.messages.extend(spec.history.iter().cloned());
            self.push(
                Role::User,
                vec![ContentBlock::text(goal_message(&spec.goal))],
            );
            self.trust(&spec.goal);
            if let Some(dir) = &spec.workdir {
                self.trust(dir);
            }
            let tools = self.registry.names();
            self.handle
                .emit(RunEvent::Started {
                    goal: spec.goal.clone(),
                    tools,
                    budget: spec.budget,
                })
                .await;
        } else {
            self.handle.emit(RunEvent::Resumed).await;
        }
        if !self.cp.pending.is_empty() {
            let results = std::mem::take(&mut self.cp.pending)
                .iter()
                .map(|tu| {
                    ContentBlock::ToolResult(skipped(
                        &tu.id,
                        "przerwane przez restart — stan nieznany, sprawdź przed ponowieniem",
                    ))
                })
                .collect();
            self.push(Role::User, results);
        }
    }

    /// Punkt atomowy: sterowanie, pauza, anulowanie, budżety.
    async fn atomic_point(&mut self) -> Option<RunOutcome> {
        loop {
            let was_paused = self.cp.paused;
            for s in self.handle.drain_steer() {
                self.apply_steer(s).await;
            }
            if self.handle.cancel.is_cancelled() {
                return Some(RunOutcome::Cancelled);
            }
            if was_paused && !self.cp.paused {
                self.handle.emit(RunEvent::Resumed).await;
            }
            if !self.cp.paused {
                break;
            }
            if !was_paused {
                self.handle.emit(RunEvent::Paused).await;
                self.checkpoint().await;
            }
            tokio::select! {
                () = self.handle.wake.notified() => {}
                () = self.handle.cancel.cancelled() => {}
            }
        }
        if let Some(budget) = self.exceeded() {
            self.handle.emit(RunEvent::BudgetExceeded { budget }).await;
            return Some(RunOutcome::BudgetExceeded { budget });
        }
        None
    }

    async fn apply_steer(&mut self, s: Steer) {
        match s {
            Steer::Message(m) => {
                let text = format!("[Wiadomość od właściciela w trakcie zadania] {m}");
                self.push(Role::User, vec![ContentBlock::text(text)]);
                self.trust(&m);
                self.handle.emit(RunEvent::Steered { message: m }).await;
            }
            Steer::ChangeGoal(g) => {
                self.push(
                    Role::User,
                    vec![ContentBlock::text(format!("[Nowy cel od właściciela] {g}"))],
                );
                self.trust(&g);
                self.cp.spec.goal = g.clone();
                self.handle.emit(RunEvent::Steered { message: g }).await;
            }
            Steer::PauseAfterCurrent => self.cp.paused = true,
            Steer::Resume => self.cp.paused = false,
            Steer::Cancel => self.handle.cancel.cancel(),
        }
    }

    async fn finish(&mut self, outcome: RunOutcome) {
        self.cp.finished = Some(outcome.clone());
        self.cp.pending.clear();
        self.checkpoint().await;
        self.handle
            .emit(RunEvent::Finished {
                outcome: outcome.clone(),
            })
            .await;
        self.handle.set_outcome(outcome);
    }

    /// Cała pętla przebiegu.
    pub(crate) async fn run(mut self) {
        self.prepare().await;
        loop {
            if let Some(outcome) = self.atomic_point().await {
                return self.finish(outcome).await;
            }
            match self.model_turn().await {
                Next::Continue => self.checkpoint().await,
                Next::Finish(outcome) => return self.finish(outcome).await,
            }
        }
    }
}
