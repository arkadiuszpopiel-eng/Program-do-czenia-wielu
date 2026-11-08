//! Granice budżetu, odmowy i eskalacja bez ponawiania pętli.
use crate::boundary::fnv;
use crate::engine::{Engine, Next};
use crate::flow::{fingerprint, repeats};
use agent_runtime_contract::{RunEvent, RunOutcome};
use providers_contract::ToolUse;
impl Engine {
    /// Kontrola przed wywołaniem: anulowanie, limit na turę, budżet, argumenty, pętla.
    pub(crate) async fn gate_call(
        &mut self,
        index: usize,
        tu: &ToolUse,
    ) -> Option<(String, Option<Next>)> {
        if let Some(error) = &self.storage_error {
            return Some((
                format!("Nie zapisano postępu: {error}"),
                Some(Next::Finish(RunOutcome::Failed {
                    error: error.clone(),
                    provider: None,
                    model: None,
                })),
            ));
        }
        if self.handle.cancel.is_cancelled() {
            return Some((
                "anulowano".into(),
                Some(Next::Finish(RunOutcome::Cancelled)),
            ));
        }
        if index >= self.cp.spec.budget.max_tool_calls_per_turn as usize {
            return Some((
                "limit wywołań w jednej turze — wywołaj ponownie w następnej".into(),
                None,
            ));
        }
        self.auto_extend().await;
        if let Some(budget) = self.exceeded() {
            self.handle.emit(RunEvent::BudgetExceeded { budget }).await;
            return Some((
                format!("przekroczony budżet {}", budget.label_pl()),
                Some(Next::Finish(RunOutcome::BudgetExceeded { budget })),
            ));
        }
        let rejected_command = self.rejected_commands.remove(&tu.id);
        // Dostawca zastępuje uszkodzony JSON pustym obiektem: to nie jest wierny odcisk
        // wywołania. Lokalne błędy gateway mają pełne wejście i mogą być liczone.
        if !rejected_command && let Some(why) = self.reject_invalid(tu).await {
            return Some((why, None));
        }
        let fp = fingerprint(&tu.name, &tu.input);
        if self.cp.denied_calls.contains(&fp) {
            return Some((
                "Operacja została odrzucona. Zmiana modelu nie zmienia uprawnień.".into(),
                Some(Next::Finish(RunOutcome::Refused)),
            ));
        }
        let n = repeats(&self.cp.recent_calls, &fp);
        if self.cp.failed_preflight.contains(&fp) || n + 1 >= self.shared.config.loop_max_repeats {
            self.handle
                .emit(RunEvent::LoopDetected {
                    tool: tu.name.clone(),
                    repeats: n + 1,
                })
                .await;
            if let Some(model) =
                self.cp.answer_model.clone().filter(|m| {
                    self.shared.provider.can_escalate_for(
                        m,
                        self.cp.route.class.as_deref().or_else(|| {
                            self.cp.spec.roles.first().map(|r| r.model_policy.as_str())
                        }),
                    )
                })
            {
                self.cp.route.escalate_from = Some(model.clone());
                self.cp.recent_calls.clear();
                self.handle.emit(RunEvent::Planned{text:format!("Wykryłam brak postępu modelu {model}. Przekazuję zapisany kontekst następnemu dopuszczonemu modelowi z kolejki zamiast powtarzać tę samą próbę.")}).await;
                return Some(("Nie powtarzaj tej samej nieskutecznej operacji. Skorzystaj z węższej metody; zadanie przejmuje kolejny model.".into(),None));
            }
            return Some((
                "wykryto pętlę (to samo wywołanie powtarza się)".into(),
                Some(Next::Finish(RunOutcome::LoopDetected {
                    tool: tu.name.clone(),
                })),
            ));
        }
        self.last_fingerprint = Some(fnv(&fp));
        self.cp.recent_calls.push(fp);
        let excess = self
            .cp
            .recent_calls
            .len()
            .saturating_sub(self.shared.config.loop_window);
        self.cp.recent_calls.drain(..excess);
        if let Some(why) = self.reject_invalid(tu).await {
            return Some((why, None));
        }

        None
    }
}
