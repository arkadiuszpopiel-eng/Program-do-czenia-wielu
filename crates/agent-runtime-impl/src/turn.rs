//! Tura modelu (plan/decyzja/weryfikacja): tool use z IR, zużycie i koszt, odpowiedź końcowa
//! → weryfikacja (Krytyczka z obsady, a gdy jej brak — samoweryfikacja v0).

use agent_runtime_contract::{RunEvent, RunOutcome, StepKind, StepStatus};
use futures_util::StreamExt;
use providers_contract::{ChatRequest, ContentBlock, Role, StopReason, ToolUse, TurnAccumulator};

use crate::engine::{Engine, Next};
use crate::prompt::{VERIFY_PROMPT, skipped, summary, system_prompt, verdict};

impl Engine {
    fn request(&self) -> ChatRequest {
        let spec = &self.cp.spec;
        let mut req = ChatRequest::new(spec.model.clone(), self.cp.messages.clone())
            .with_system(system_prompt(spec));
        req.tools = self.registry.specs(self.cp.verifying);
        req.params.max_tokens = self.shared.config.max_output_tokens;
        req.meta.session = Some(spec.session.as_str().to_owned());
        req
    }

    /// Tura modelu.
    pub(crate) async fn model_turn(&mut self) -> Next {
        let kind = if self.cp.verifying {
            StepKind::Verify
        } else if self.cp.planned {
            StepKind::Think
        } else {
            StepKind::Plan
        };
        self.cp.usage.steps += 1;
        let step = self.cp.usage.steps;
        let input = self
            .cp
            .messages
            .last()
            .map(|m| summary(&m.visible_text(), 160))
            .unwrap_or_default();
        self.handle
            .emit(RunEvent::StepStarted {
                step,
                kind,
                tool: None,
                input: input.clone(),
            })
            .await;
        let provider = self.shared.provider.clone();
        let mut stream = provider.stream(self.request(), self.handle.cancel.child_token());
        let mut acc = TurnAccumulator::new(provider.id().clone());
        while let Some(ev) = stream.next().await {
            acc.push(&ev);
        }
        let turn = acc.finish();
        self.cp.usage.input_tokens += turn.usage.total_input();
        self.cp.usage.output_tokens += turn.usage.output_tokens;
        if let Some(cost) = provider.cost(&self.cp.spec.model, &turn.usage) {
            self.cp.usage.cost_nano_usd = self.cp.usage.cost_nano_usd.saturating_add(cost.nano_usd);
        }
        self.cp.usage.elapsed_ms = self.elapsed_ms();
        self.handle.emit(RunEvent::Usage(self.cp.usage)).await;
        let finished = |status, output: String| RunEvent::StepFinished {
            step,
            kind,
            tool: None,
            status,
            input: input.clone(),
            output,
            untrusted: false,
            undo: None,
            intent: None,
            approval: None,
        };
        if self.handle.cancel.is_cancelled() || turn.stop == Some(StopReason::Cancelled) {
            self.handle
                .emit(finished(StepStatus::Cancelled, String::new()))
                .await;
            return Next::Finish(RunOutcome::Cancelled);
        }
        if let Some(err) = &turn.error {
            self.handle
                .emit(finished(StepStatus::Failed, err.to_string()))
                .await;
            return Next::Finish(RunOutcome::Failed {
                error: err.to_string(),
            });
        }
        match turn.stop {
            Some(StopReason::Refusal) => {
                self.handle
                    .emit(finished(StepStatus::Denied, "odmowa modelu".into()))
                    .await;
                return Next::Finish(RunOutcome::Refused);
            }
            Some(StopReason::ContextWindowExceeded) => {
                self.handle
                    .emit(finished(
                        StepStatus::Failed,
                        "przekroczone okno kontekstu".into(),
                    ))
                    .await;
                return Next::Finish(RunOutcome::Failed {
                    error: "przekroczone okno kontekstu modelu".into(),
                });
            }
            _ => {}
        }
        let text = turn.message.visible_text();
        let uses: Vec<ToolUse> = turn.message.tool_uses().cloned().collect();
        if !turn.message.content.is_empty() {
            self.cp.messages.push(turn.message.clone());
        }
        let names: Vec<&str> = uses.iter().map(|u| u.name.as_str()).collect();
        let output = if names.is_empty() {
            summary(&text, 300)
        } else {
            format!("{} → {}", summary(&text, 200), names.join(", "))
        };
        self.handle.emit(finished(StepStatus::Ok, output)).await;
        if !self.cp.planned {
            self.cp.planned = true;
            if !text.trim().is_empty() {
                self.handle
                    .emit(RunEvent::Planned {
                        text: summary(&text, 2000),
                    })
                    .await;
            }
        }
        if turn.stop == Some(StopReason::MaxTokens) && !uses.is_empty() {
            let results = uses
                .iter()
                .map(|u| {
                    ContentBlock::ToolResult(skipped(
                        &u.id,
                        "odpowiedź ucięta limitem tokenów, argumenty niepełne — spróbuj krócej",
                    ))
                })
                .collect();
            self.push(Role::User, results);
            return Next::Continue;
        }
        if uses.is_empty() {
            return self.final_answer(text).await;
        }
        self.run_tools(uses).await
    }

    /// Odpowiedź końcowa: weryfikacja (Krytyczka ≠ autorka, gdy obsada na to pozwala).
    async fn final_answer(&mut self, text: String) -> Next {
        if self.cp.verifying {
            let ok = verdict(&text);
            self.handle
                .emit(RunEvent::Verified {
                    ok: ok.unwrap_or(false),
                    note: summary(&text, 300),
                })
                .await;
            if ok == Some(false) && self.cp.verify_rounds < self.shared.config.max_verify_rounds {
                self.cp.verifying = false;
                self.push(
                    Role::User,
                    vec![ContentBlock::text(
                        "[Weryfikacja] Wykryto problem — popraw wynik, a potem zakończ.",
                    )],
                );
                return Next::Continue;
            }
            return Next::Finish(RunOutcome::Completed {
                summary: text,
                verified: ok,
            });
        }
        if !self.cp.spec.verify {
            return Next::Finish(RunOutcome::Completed {
                summary: text,
                verified: None,
            });
        }
        if let Some(critic) = self.critic_for_run() {
            return self.critic_review(critic, text).await;
        }
        self.cp.verifying = true;
        self.cp.verify_rounds += 1;
        self.push(Role::User, vec![ContentBlock::text(VERIFY_PROMPT)]);
        Next::Continue
    }
}
