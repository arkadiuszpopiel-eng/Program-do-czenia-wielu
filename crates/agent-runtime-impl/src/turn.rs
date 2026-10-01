//! Tura modelu (plan/decyzja/weryfikacja) i wykonanie narzędzi z obserwacją: tool use z IR,
//! zużycie i koszt, wyniki narzędzi jako niezaufana treść, taint, proweniencja, pętla.

use std::sync::Arc;

use agent_runtime_contract::{RunEvent, RunOutcome, StepKind, StepStatus};
use futures_util::StreamExt;
use providers_contract::{ChatRequest, ContentBlock, Role, StopReason, ToolUse, TurnAccumulator};
use safety_broker_contract::{ApprovalId, ApprovalTicket, Holder};
use tools_common_contract::{ToolCtx, ToolErrorKind, ToolObserver, ToolOutcome, ToolStatus};

use crate::engine::{Engine, Next};
use crate::flow::{append_capped, args_untrusted, fingerprint, repeats};
use crate::handle::RunHandle;
use crate::prompt::{VERIFY_PROMPT, skipped, summary, system_prompt, tool_result, verdict};

/// Obserwator wywołania: karta „czeka na zatwierdzenie” w UI.
struct StepObserver {
    handle: Arc<RunHandle>,
    step: u32,
}

impl ToolObserver for StepObserver {
    fn approval_requested(&self, ticket: &ApprovalTicket) {
        self.handle.emit_detached(RunEvent::WaitingApproval {
            step: self.step,
            approval: ticket.id,
            explanation: ticket.explanation.clone(),
        });
    }

    fn approval_resolved(&self, _id: ApprovalId, _approved: bool) {}
}

fn step_status(s: &ToolStatus) -> StepStatus {
    match s {
        ToolStatus::Ok => StepStatus::Ok,
        ToolStatus::Denied { .. } => StepStatus::Denied,
        ToolStatus::NeedsConfirmation => StepStatus::NeedsConfirmation,
        ToolStatus::Failed { .. } => StepStatus::Failed,
        ToolStatus::Cancelled => StepStatus::Cancelled,
    }
}

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
        if self.cp.spec.verify {
            self.cp.verifying = true;
            self.cp.verify_rounds += 1;
            self.push(Role::User, vec![ContentBlock::text(VERIFY_PROMPT)]);
            return Next::Continue;
        }
        Next::Finish(RunOutcome::Completed {
            summary: text,
            verified: None,
        })
    }

    fn ctx(&self, step: u32, args: &serde_json::Value, title: &str) -> ToolCtx {
        let spec = &self.cp.spec;
        let holder = Holder {
            session: spec.session.clone(),
            agent: Some(spec.agent.clone()),
            role: spec.roles.first().map(|r| r.id.as_str().to_owned()),
        };
        let mut ctx = ToolCtx::new(holder);
        ctx.origin = spec.origin;
        ctx.run = Some(self.handle.run.clone());
        ctx.step = step;
        ctx.label = format!("{}: {}", spec.persona.name, title.to_lowercase());
        ctx.workdir = spec.workdir.clone();
        ctx.untrusted_args = args_untrusted(args, &self.cp.trusted_text, &self.cp.untrusted_text);
        ctx.cancel = self.handle.cancel.child_token();
        ctx.approval_timeout = std::time::Duration::from_millis(spec.approval_timeout_ms);
        ctx.observer = Some(Arc::new(StepObserver {
            handle: self.handle.clone(),
            step,
        }));
        ctx
    }

    /// Kontrola przed wywołaniem: anulowanie, limit na turę, budżet, pętla.
    async fn gate_call(
        &mut self,
        index: usize,
        tu: &ToolUse,
    ) -> Option<(String, Option<RunOutcome>)> {
        if self.handle.cancel.is_cancelled() {
            return Some(("anulowano".into(), Some(RunOutcome::Cancelled)));
        }
        if index >= self.cp.spec.budget.max_tool_calls_per_turn as usize {
            return Some((
                "limit wywołań w jednej turze — wywołaj ponownie w następnej".into(),
                None,
            ));
        }
        if let Some(budget) = self.exceeded() {
            self.handle.emit(RunEvent::BudgetExceeded { budget }).await;
            return Some((
                format!("przekroczony budżet {}", budget.label_pl()),
                Some(RunOutcome::BudgetExceeded { budget }),
            ));
        }
        let fp = fingerprint(&tu.name, &tu.input);
        let n = repeats(&self.cp.recent_calls, &fp);
        if n + 1 >= self.shared.config.loop_max_repeats {
            self.handle
                .emit(RunEvent::LoopDetected {
                    tool: tu.name.clone(),
                    repeats: n + 1,
                })
                .await;
            return Some((
                "wykryto pętlę (to samo wywołanie powtarza się)".into(),
                Some(RunOutcome::LoopDetected {
                    tool: tu.name.clone(),
                }),
            ));
        }
        self.cp.recent_calls.push(fp);
        let excess = self
            .cp
            .recent_calls
            .len()
            .saturating_sub(self.shared.config.loop_window);
        self.cp.recent_calls.drain(..excess);
        None
    }

    async fn call_tool(&mut self, tu: &ToolUse) -> ToolOutcome {
        self.cp.usage.steps += 1;
        self.cp.usage.tool_calls += 1;
        let step = self.cp.usage.steps;
        let input = summary(&tu.input.to_string(), 200);
        let tool = self.registry.get(&tu.name).cloned();
        let title = tool
            .as_ref()
            .map_or_else(|| tu.name.clone(), |t| t.manifest().title.clone());
        self.handle
            .emit(RunEvent::StepStarted {
                step,
                kind: StepKind::Tool,
                tool: Some(tu.name.clone()),
                input: input.clone(),
            })
            .await;
        let outcome = match tool {
            Some(t) => {
                t.call(tu.input.clone(), &self.ctx(step, &tu.input, &title))
                    .await
            }
            None => ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!(
                    "Nieznane albo niedostępne narzędzie `{}`. Dostępne: {}.",
                    tu.name,
                    self.registry.names().join(", ")
                ),
            ),
        };
        if let Some(source) = outcome.untrusted.clone() {
            if !self.cp.tainted {
                self.cp.tainted = true;
                self.handle.emit(RunEvent::Tainted { source }).await;
            }
            if !self.shared.config.metadata_tools.contains(&tu.name) {
                let cap = self.shared.config.provenance_cap;
                append_capped(&mut self.cp.untrusted_text, &outcome.text, cap);
            }
        }
        self.handle
            .emit(RunEvent::StepFinished {
                step,
                kind: StepKind::Tool,
                tool: Some(tu.name.clone()),
                status: step_status(&outcome.status),
                input,
                output: summary(&outcome.text, 300),
                untrusted: outcome.untrusted.is_some(),
                undo: outcome.undo.clone(),
                intent: outcome.intent.clone(),
                approval: outcome.approval,
            })
            .await;
        outcome
    }

    /// Narzędzia tury — sekwencyjnie (v0); checkpoint przed akcjami (wywołania w toku).
    async fn run_tools(&mut self, uses: Vec<ToolUse>) -> Next {
        self.cp.pending = uses.clone();
        self.checkpoint().await;
        let mut results = Vec::with_capacity(uses.len());
        let mut stop: Option<RunOutcome> = None;
        for (i, tu) in uses.iter().enumerate() {
            if let Some(o) = &stop {
                let why = if *o == RunOutcome::Cancelled {
                    "anulowano"
                } else {
                    "przebieg zatrzymany"
                };
                results.push(ContentBlock::ToolResult(skipped(&tu.id, why)));
                continue;
            }
            if let Some((why, outcome)) = self.gate_call(i, tu).await {
                results.push(ContentBlock::ToolResult(skipped(&tu.id, &why)));
                stop = outcome;
                continue;
            }
            let outcome = self.call_tool(tu).await;
            let block_id = format!("{}-{}", self.cp.usage.steps, i);
            let max = self.shared.config.tool_result_max_chars;
            results.push(ContentBlock::ToolResult(tool_result(
                &tu.id, &tu.name, &outcome, &block_id, max,
            )));
            if outcome.status == ToolStatus::Cancelled && self.handle.cancel.is_cancelled() {
                stop = Some(RunOutcome::Cancelled);
            }
        }
        self.push(Role::User, results);
        self.cp.pending.clear();
        match stop {
            Some(o) => Next::Finish(o),
            None => Next::Continue,
        }
    }
}
