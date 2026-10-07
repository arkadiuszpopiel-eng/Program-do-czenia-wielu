//! Granica kroku atomowego (F5, PLAN §9.6): po każdym ukończonym kroku — także przed każdym
//! wywołaniem narzędzia w turze — runtime pyta scheduler (`StepGate::boundary`: steering,
//! oddanie przy pauzie/wywłaszczeniu, zatrzymanie) i sprawdza własną kolejkę sterowania.
//! Nowa wiadomość właściciela przerywa resztę narzędzi tury, więc trafia do agentki w ≤ 1 kroku.

use agent_runtime_contract::{BudgetKind, RunEvent, RunOutcome, Steer};
use scheduler_contract::{
    BudgetKind as TaskBudget, Steer as TaskSteer, SteerEnvelope, SteerVia, StepDirective,
    StepReport, StopReason,
};

use crate::engine::{Engine, Next};

/// Wynik granicy przed wywołaniem narzędzia.
pub(crate) enum Boundary {
    /// Wywołuj dalej.
    Go,
    /// Pomiń resztę wywołań tury (powód dla modelu); sterowanie trafi do następnej tury.
    SkipRest(&'static str),
    /// Zakończ przebieg.
    Stop(RunOutcome),
    /// Oddaj zadanie schedulerowi.
    Yield,
}

/// Powód pominięcia po nowej wiadomości właściciela.
pub(crate) const SKIP_STEERED: &str =
    "właściciel przysłał nową wiadomość — przeplanuj z jej uwzględnieniem";
/// Powód pominięcia po oddaniu zadania schedulerowi.
pub(crate) const SKIP_YIELDED: &str =
    "zadanie oddane schedulerowi (pauza albo pierwszeństwo innego zadania) — wznowisz później";

/// Dyrektywa „stop” schedulera → wynik przebiegu.
fn stop_outcome(reason: &StopReason) -> RunOutcome {
    match reason {
        StopReason::Cancelled { .. } => RunOutcome::Cancelled,
        StopReason::Budget { budget } => RunOutcome::BudgetExceeded {
            budget: match budget {
                TaskBudget::Steps => BudgetKind::Steps,
                TaskBudget::Wall => BudgetKind::Wall,
                TaskBudget::Cost => BudgetKind::Cost,
            },
        },
        StopReason::Deadline => RunOutcome::BudgetExceeded {
            budget: BudgetKind::Wall,
        },
    }
}

/// Odcisk kroku dla schedulera (FNV-1a — deterministyczny między uruchomieniami).
pub(crate) fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl Engine {
    /// Sterowanie ze schedulera do kolejki przebiegu (treść; operacje scheduler robi sam).
    pub(crate) fn queue_task_steering(&self, steering: Vec<SteerEnvelope>) {
        for env in steering {
            match env.steer {
                TaskSteer::Message { text, via } => self
                    .handle
                    .push_steer(Steer::Message(text), via == SteerVia::Voice),
                TaskSteer::ChangeGoal { goal, via } => self
                    .handle
                    .push_steer(Steer::ChangeGoal(goal), via == SteerVia::Voice),
                TaskSteer::PauseAfterCurrent | TaskSteer::Resume | TaskSteer::Cancel => {}
            }
        }
    }

    /// `StepGate::boundary` po ukończonym kroku (raz na krok; bez schedulera — nic).
    pub(crate) async fn gate_boundary(&mut self) -> Next {
        let Some(gate) = self.hooks.gate.clone() else {
            return Next::Continue;
        };
        if self.cp.usage.steps <= self.boundary_at {
            return Next::Continue;
        }
        let cost_nano = self
            .cp
            .usage
            .cost_nano_usd
            .saturating_sub(self.cost_at_boundary);
        let report = StepReport {
            cost_micro_pln: cost_nano
                .saturating_mul(self.hooks.usd_pln_e4)
                .checked_div(10_000_000)
                .unwrap_or(0),
            fingerprint: self.last_fingerprint.take(),
        };
        self.boundary_at = self.cp.usage.steps;
        self.cost_at_boundary = self.cp.usage.cost_nano_usd;
        match gate.boundary(report) {
            StepDirective::Continue { steering } => {
                self.queue_task_steering(steering);
                Next::Continue
            }
            StepDirective::Yield { .. } => Next::Yield,
            StepDirective::Stop { reason } => {
                self.gate_stopped = true;
                let outcome = stop_outcome(&reason);
                if let RunOutcome::BudgetExceeded { budget } = &outcome {
                    let budget = *budget;
                    self.handle.emit(RunEvent::BudgetExceeded { budget }).await;
                }
                Next::Finish(outcome)
            }
        }
    }

    /// Granica przed wywołaniem narzędzia (albo paczką równoległych odczytów).
    pub(crate) async fn tool_boundary(&mut self) -> Boundary {
        match self.gate_boundary().await {
            Next::Continue => {}
            Next::Finish(outcome) => return Boundary::Stop(outcome),
            Next::Yield => return Boundary::Yield,
        }
        for op in self.handle.take_ops() {
            self.apply_op(op);
        }
        if self.handle.cancel.is_cancelled() {
            return Boundary::Stop(RunOutcome::Cancelled);
        }
        if self.cp.paused && !self.wait_while_paused().await {
            return Boundary::Stop(RunOutcome::Cancelled);
        }
        if self.handle.has_content() {
            return Boundary::SkipRest(SKIP_STEERED);
        }
        Boundary::Go
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_contract::CancelCause;

    #[test]
    fn stop_reasons_map_to_outcomes() {
        assert_eq!(
            stop_outcome(&StopReason::Cancelled {
                cause: CancelCause::KillSwitch
            }),
            RunOutcome::Cancelled
        );
        assert_eq!(
            stop_outcome(&StopReason::Budget {
                budget: TaskBudget::Cost
            }),
            RunOutcome::BudgetExceeded {
                budget: BudgetKind::Cost
            }
        );
        assert_eq!(
            stop_outcome(&StopReason::Deadline),
            RunOutcome::BudgetExceeded {
                budget: BudgetKind::Wall
            }
        );
        assert_eq!(fnv("a"), fnv("a"));
        assert_ne!(fnv("a"), fnv("b"));
    }
}
