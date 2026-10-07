//! Pętla przebiegu: punkt atomowy (granica kroku schedulera, sterowanie, pauza, anulowanie,
//! budżety, checkpoint) → tura modelu → narzędzia → … → weryfikacja (Krytyczka albo
//! samoweryfikacja) → wynik. Każdy krok jest zdarzeniem.

use std::sync::Arc;

use agent_runtime_contract::{BudgetKind, Checkpoint, RunEvent, RunOutcome, Steer};
use providers_contract::{ContentBlock, Message, Role};

use crate::flow::append_capped;
use crate::handle::{Queued, RunHandle};
use crate::prompt::{goal_message, skipped};
use crate::registry::ToolRegistry;
use crate::shared::{Exit, Hooks, Shared};

/// Co dalej po turze.
pub(crate) enum Next {
    Continue,
    Finish(RunOutcome),
    /// Oddanie zadania schedulerowi (pauza/wywłaszczenie w punkcie atomowym).
    Yield,
}

pub(crate) struct Engine {
    pub(crate) shared: Arc<Shared>,
    pub(crate) handle: Arc<RunHandle>,
    pub(crate) cp: Checkpoint,
    pub(crate) registry: ToolRegistry,
    pub(crate) hooks: Hooks,
    started: tokio::time::Instant,
    base_elapsed: u64,
    /// Kroki ukończone przy ostatnim wywołaniu `StepGate::boundary`.
    pub(crate) boundary_at: u32,
    /// Koszt (nano-USD) przy ostatnim wywołaniu granicy.
    pub(crate) cost_at_boundary: u64,
    /// Odcisk ostatniego kroku narzędzia (wykrywanie pętli przez scheduler).
    pub(crate) last_fingerprint: Option<u64>,
    /// Scheduler zażądał oddania zadania w trakcie podprzebiegu.
    pub(crate) yield_requested: bool,
    /// Scheduler zatrzymał zadanie (`StepDirective::Stop`).
    pub(crate) gate_stopped: bool,
}

impl Engine {
    pub(crate) fn new(
        shared: Arc<Shared>,
        handle: Arc<RunHandle>,
        cp: Checkpoint,
        registry: ToolRegistry,
        hooks: Hooks,
    ) -> Self {
        let base_elapsed = cp.usage.elapsed_ms;
        let boundary_at = cp.usage.steps;
        let cost_at_boundary = cp.usage.cost_nano_usd;
        Self {
            shared,
            handle,
            cp,
            registry,
            hooks,
            started: tokio::time::Instant::now(),
            base_elapsed,
            boundary_at,
            cost_at_boundary,
            last_fingerprint: None,
            yield_requested: false,
            gate_stopped: false,
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
        let d = &self.cp.delegated;
        if u.steps.saturating_add(d.steps) >= b.max_steps {
            return Some(BudgetKind::Steps);
        }
        if u.tokens().saturating_add(d.tokens()) >= b.max_tokens {
            return Some(BudgetKind::Tokens);
        }
        if self.elapsed_ms() >= b.max_wall_ms {
            return Some(BudgetKind::Wall);
        }
        let cost = u.cost_nano_usd.saturating_add(d.cost_nano_usd);
        match b.max_cost_micro_usd {
            Some(max) if cost / 1000 >= max => Some(BudgetKind::Cost),
            _ => None,
        }
    }

    /// Start świeży albo po restarcie/oddaniu: wiadomość celu, wyniki dla przerwanych wywołań.
    async fn prepare(&mut self) {
        if self.cp.messages.is_empty() {
            let spec = self.cp.spec.clone();
            self.cp.messages.extend(spec.history.iter().cloned());
            self.push(
                Role::User,
                vec![ContentBlock::text(goal_message(
                    &spec.goal,
                    self.cp.options.parent.is_some(),
                ))],
            );
            if self.cp.options.parent.is_none() {
                // Cel od właściciela jest zaufany; cel podprzebiegu napisał model rodzica.
                self.trust(&spec.goal);
            }
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
            if let Some(source) = self.cp.taint_source.clone() {
                self.handle.emit(RunEvent::Tainted { source }).await;
            }
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

    /// Czeka w pauzie (użytkownika) na wznowienie albo anulowanie; `false` = anulowano.
    pub(crate) async fn wait_while_paused(&mut self) -> bool {
        self.handle.emit(RunEvent::Paused).await;
        loop {
            tokio::select! {
                () = self.handle.wake.notified() => {}
                () = self.handle.cancel.cancelled() => return false,
            }
            for op in self.handle.take_ops() {
                self.apply_op(op);
            }
            if self.handle.cancel.is_cancelled() {
                return false;
            }
            if !self.cp.paused {
                self.handle.emit(RunEvent::Resumed).await;
                return true;
            }
        }
    }

    /// Punkt atomowy przed turą modelu: granica schedulera, sterowanie, pauza, anulowanie, budżety.
    async fn atomic_point(&mut self) -> Next {
        match self.gate_boundary().await {
            Next::Continue => {}
            other => return other,
        }
        loop {
            let was_paused = self.cp.paused;
            for s in self.handle.drain_steer() {
                self.apply_steer(s).await;
            }
            if self.handle.cancel.is_cancelled() {
                return Next::Finish(RunOutcome::Cancelled);
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
            return Next::Finish(RunOutcome::BudgetExceeded { budget });
        }
        Next::Continue
    }

    pub(crate) fn apply_op(&mut self, s: Steer) {
        match s {
            Steer::PauseAfterCurrent => self.cp.paused = true,
            Steer::Resume => self.cp.paused = false,
            Steer::Cancel => self.handle.cancel.cancel(),
            Steer::Message(_) | Steer::ChangeGoal(_) => {}
        }
    }

    async fn apply_steer(&mut self, q: Queued) {
        let who = if q.voice {
            "Wiadomość głosowa od właściciela w trakcie zadania"
        } else {
            "Wiadomość od właściciela w trakcie zadania"
        };
        match q.steer {
            Steer::Message(m) => {
                let text = format!("[{who}] {m}");
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
            op => self.apply_op(op),
        }
    }

    async fn finish(&mut self, outcome: RunOutcome) -> Exit {
        self.cp.finished = Some(outcome.clone());
        self.cp.pending.clear();
        self.checkpoint().await;
        self.handle
            .emit(RunEvent::Finished {
                outcome: outcome.clone(),
            })
            .await;
        self.handle.set_outcome(outcome.clone());
        if self.gate_stopped {
            Exit::Stopped(outcome)
        } else {
            Exit::Finished(outcome)
        }
    }

    /// Oddanie schedulerowi: checkpoint, `Paused` w dzienniku, uchwyt czeka na ponowny start.
    async fn suspend(&mut self) -> Exit {
        self.cp.pending.clear();
        self.checkpoint().await;
        self.handle.emit(RunEvent::Paused).await;
        self.handle.set_running(false);
        Exit::Yielded
    }

    /// Cała pętla przebiegu.
    pub(crate) async fn run(mut self) -> Exit {
        let initial = std::mem::take(&mut self.hooks.initial_steering);
        self.queue_task_steering(initial);
        self.prepare().await;
        loop {
            match self.atomic_point().await {
                Next::Continue => {}
                Next::Finish(outcome) => return self.finish(outcome).await,
                Next::Yield => return self.suspend().await,
            }
            match self.model_turn().await {
                Next::Continue => self.checkpoint().await,
                Next::Finish(outcome) => return self.finish(outcome).await,
                Next::Yield => return self.suspend().await,
            }
        }
    }
}
