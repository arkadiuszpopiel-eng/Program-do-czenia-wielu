//! Wspólny budżet orkiestracji; oczekiwanie nie zużywa czasu aktywnego.
use agent_runtime_contract::{BudgetKind, ContinuationPolicy, RunBudget, RunEvent, UsageTotals};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;
pub(crate) struct ConductorBudget {
    limits: Mutex<RunBudget>,
    storage_error: Mutex<Option<String>>,
    policy: ContinuationPolicy,
    waiting: app_agents::ApprovalClock,
    paused: AtomicBool,
    started: Instant,
    steps: AtomicU32,
    tokens: AtomicU64,
    cost_nano: AtomicU64,
}
impl ConductorBudget {
    pub fn new(limits: RunBudget, started: Instant) -> Self {
        Self {
            limits: Mutex::new(limits),
            storage_error: Mutex::new(None),
            policy: Default::default(),
            waiting: Default::default(),
            paused: AtomicBool::new(false),
            started,
            steps: AtomicU32::new(0),
            tokens: AtomicU64::new(0),
            cost_nano: AtomicU64::new(0),
        }
    }
    pub fn with_policy(mut self, policy: ContinuationPolicy) -> Self {
        self.policy = policy;
        self
    }
    pub fn charge_turn(&self, turn: &providers_contract::AssistantTurn, cost_nano_usd: u64) {
        self.charge(UsageTotals {
            steps: 1,
            input_tokens: turn.usage.total_input(),
            output_tokens: turn.usage.output_tokens,
            cost_nano_usd,
            ..UsageTotals::default()
        });
    }
    pub fn charge(&self, u: UsageTotals) {
        self.steps.fetch_add(u.steps, Ordering::Relaxed);
        self.tokens.fetch_add(u.tokens(), Ordering::Relaxed);
        self.cost_nano.fetch_add(u.cost_nano_usd, Ordering::Relaxed);
    }
    pub fn fail_storage(&self, error: impl ToString) {
        *self
            .storage_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(error.to_string());
        self.pause();
    }
    pub fn storage_error(&self) -> Option<String> {
        self.storage_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
    pub fn limits(&self) -> RunBudget {
        *self.limits.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn steps(&self) -> u32 {
        self.steps.load(Ordering::Relaxed)
    }
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
    pub fn auto_for(&self, k: BudgetKind) -> bool {
        match k {
            BudgetKind::Steps | BudgetKind::Tokens => self.policy.auto_steps,
            BudgetKind::Wall => self.policy.auto_time,
            _ => false,
        }
    }
    pub fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .saturating_sub(self.waiting.waited_ms())
    }
    pub fn observe(&self, event: &RunEvent) {
        match event {
            RunEvent::WaitingApproval { approval, .. } => {
                self.waiting.begin(format!("{approval:?}"))
            }
            RunEvent::ApprovalResolved { approval } => self.waiting.end(&format!("{approval:?}")),
            _ => {}
        }
    }
    pub fn remaining(&self) -> Option<RunBudget> {
        if self.paused.load(Ordering::Relaxed) {
            return None;
        }
        let mut b = self.limits.lock().unwrap_or_else(PoisonError::into_inner);
        let cost = self.cost_nano.load(Ordering::Relaxed).div_ceil(1000);
        let max_cost_micro_usd = b.max_cost_micro_usd.map(|v| v.saturating_sub(cost));
        if max_cost_micro_usd == Some(0) {
            return None;
        }
        if self.policy.auto_steps
            && (self.steps() >= b.max_steps || self.tokens.load(Ordering::Relaxed) >= b.max_tokens)
        {
            b.max_steps = b.max_steps.saturating_add(self.policy.step_chunk.max(1));
            b.max_tokens = b
                .max_tokens
                .saturating_add(u64::from(self.policy.step_chunk.max(1)) * 50_000);
        }
        let elapsed = self.elapsed_ms();
        if self.policy.auto_time && elapsed >= b.max_wall_ms {
            b.max_wall_ms = b
                .max_wall_ms
                .saturating_add(self.policy.time_chunk_ms.max(60_000));
        }
        let max_steps = b.max_steps.saturating_sub(self.steps());
        let max_tokens = b
            .max_tokens
            .saturating_sub(self.tokens.load(Ordering::Relaxed));
        let max_wall_ms = b.max_wall_ms.saturating_sub(elapsed);
        (max_steps > 0 && max_tokens > 0 && max_wall_ms > 0).then_some(RunBudget {
            max_steps,
            max_tokens,
            max_wall_ms,
            max_cost_micro_usd,
            max_tool_calls_per_turn: b.max_tool_calls_per_turn,
        })
    }
    pub fn step(&self) -> bool {
        if self.remaining().is_none() {
            return false;
        }
        self.charge(UsageTotals {
            steps: 1,
            ..UsageTotals::default()
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn children_share_500_steps_instead_of_restarting_the_budget() {
        let meter = ConductorBudget::new(
            RunBudget {
                max_steps: 500,
                ..RunBudget::default()
            },
            Instant::now(),
        );
        meter.charge(UsageTotals {
            steps: 301,
            ..UsageTotals::default()
        });
        assert_eq!(meter.remaining().map(|b| b.max_steps), Some(199));
        meter.charge(UsageTotals {
            steps: 199,
            ..UsageTotals::default()
        });
        assert!(meter.remaining().is_none());
    }
    #[test]
    fn cost_and_wall_are_shared_too() {
        let meter = ConductorBudget::new(
            RunBudget {
                max_cost_micro_usd: Some(3),
                ..RunBudget::default()
            },
            Instant::now(),
        );
        meter.charge(UsageTotals {
            cost_nano_usd: 3000,
            ..UsageTotals::default()
        });
        assert!(meter.remaining().is_none());
        let meter = ConductorBudget::new(
            RunBudget {
                max_wall_ms: 1,
                ..RunBudget::default()
            },
            Instant::now() - std::time::Duration::from_secs(1),
        );
        assert!(meter.remaining().is_none());
    }
}
