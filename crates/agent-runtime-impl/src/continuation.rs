//! Wznowienie wyłącznie miękkiego limitu, nigdy anulowania lub odmowy.
use crate::Runtime;
use agent_runtime_contract::{BudgetKind, RunBudget, RunError, RunId, RunOutcome};
impl Runtime {
    /// Nowa pula przyznana przez właściciela; historia i efekty operacji pozostają.
    pub async fn extend_and_resume(&self, run: &RunId, budget: RunBudget) -> Result<(), RunError> {
        let mut cp = self
            .shared
            .store
            .latest(run)
            .map_err(|e| RunError::Store(e.to_string()))?
            .ok_or_else(|| RunError::NoCheckpoint(run.clone()))?;
        if !matches!(cp.finished, Some(RunOutcome::BudgetExceeded { .. })) {
            return Err(RunError::AlreadyFinished(run.clone()));
        }
        cp.spec.budget = budget;
        if let Some(grant) = cp.options.grant.as_mut() {
            grant.budget = cp.spec.budget;
        }
        cp.finished = None;
        cp.paused = false;
        cp.messages.push(providers_contract::Message::user_text("Właściciel zgodził się na kolejną pulę pracy. Kontynuuj od zapisanego miejsca; nie ponawiaj zakończonych operacji. Jeśli operacja była rozpoczęta i ma nieznany stan, najpierw go sprawdź."));
        self.shared
            .store
            .save(&cp)
            .map_err(|e| RunError::Store(e.to_string()))?;
        self.launch(cp).map(|_| ())
    }
}
impl crate::engine::Engine {
    pub(crate) async fn auto_extend(&mut self) -> bool {
        if self.cp.options.depth > 0 {
            return false;
        }
        let policy = &self.cp.options.continuation;
        let kind = self.exceeded();
        let changed = match kind {
            Some(BudgetKind::Steps | BudgetKind::Tokens) if policy.auto_steps => {
                let chunk = policy.step_chunk.max(1);
                let old = self.cp.spec.budget.max_steps;
                self.cp.spec.budget.max_steps = old.saturating_add(chunk);
                self.cp.spec.budget.max_tokens = self
                    .cp
                    .spec
                    .budget
                    .max_tokens
                    .saturating_add(u64::from(chunk) * 50_000);
                old < self.cp.spec.budget.max_steps
            }
            Some(BudgetKind::Wall) if policy.auto_time => {
                let old = self.cp.spec.budget.max_wall_ms;
                self.cp.spec.budget.max_wall_ms =
                    old.saturating_add(policy.time_chunk_ms.max(60_000));
                old < self.cp.spec.budget.max_wall_ms
            }
            _ => false,
        };
        if changed {
            if let Some(grant) = self.cp.options.grant.as_mut() {
                grant.budget = self.cp.spec.budget;
            }
            self.handle.emit(agent_runtime_contract::RunEvent::Planned{text:format!("Automatycznie przedłużyłam limit {} zgodnie z ustawieniem właściciela; stan zadania jest zapisany.",kind.map_or("zadania",|k|k.label_pl()))}).await;
            self.checkpoint().await;
        }
        changed
    }
}
