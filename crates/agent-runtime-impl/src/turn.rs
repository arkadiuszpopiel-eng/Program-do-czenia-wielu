//! Tura modelu (plan/decyzja/weryfikacja): tool use z IR, zużycie i koszt, odpowiedź końcowa
//! → weryfikacja (Krytyczka z obsady, a gdy jej brak — samoweryfikacja v0).

use std::collections::BTreeMap;

use agent_runtime_contract::{RunEvent, RunOutcome, StepKind, StepStatus};
use futures_util::StreamExt;
use providers_contract::{ChatRequest, ContentBlock, Role, StopReason, TurnAccumulator};

use crate::engine::{Engine, Next};
use crate::prompt::{VERIFY_PROMPT, skipped, summary, system_prompt_for, verdict};

impl Engine {
    fn request(&self) -> ChatRequest {
        let spec = &self.cp.spec;
        let (messages, shortened) = if self.registry.names().is_empty() {
            (self.cp.messages.clone(), false)
        } else {
            crate::context::view(&self.cp.messages)
        };
        let mut req = ChatRequest::new(spec.model.clone(), messages)
            .with_system(system_prompt_for(spec, self.cp.verifying));
        req.tools = self
            .registry
            .specs(self.cp.verifying, &spec.goal, &self.cp.discovered_tools);
        if req.tools.iter().any(|t| t.name == crate::catalog::FIND) {
            req.system
                .get_or_insert_with(String::new)
                .push_str(crate::catalog::QUICK_GUIDE);
        }
        if shortened {
            if req.tools.len() >= crate::catalog::VISIBLE_LIMIT
                && let Some(index) = req
                    .tools
                    .iter()
                    .rposition(|t| t.name != crate::catalog::FIND && t.name != crate::catalog::CALL)
            {
                req.tools.remove(index);
            }
            req.tools.push(crate::context::spec());
            req.system.get_or_insert_with(String::new).push_str("\nStarsze wyniki narzędzi mogą być skrócone; pełny lokalny zapis odczytaj przez task_recall. Zachowaj wszystkie instrukcje właściciela.");
        }
        req.params.max_tokens = self.shared.config.max_output_tokens;
        // Weryfikacja z temperaturą modelu — stały format odpowiedzi kontrolnej.
        req.params.temperature = spec.temperature.filter(|_| !self.cp.verifying);
        req.meta.session = Some(spec.session.as_str().to_owned());
        req.meta.privacy.tag = self.cp.options.request_privacy;
        // Trudność oceniana z celu przebiegu, nie z ostatniej wiadomości (wyniki narzędzi).
        let mut hint = self.cp.route.clone();
        if !hint.review && hint.difficulty.is_none() && hint.task.is_none() {
            hint.set_task(&spec.goal);
        }
        req.meta.route = hint;
        if req.meta.route.escalate_from.is_some() && self.shared.provider.id().as_str() == "router"
        {
            req.model = "auto".into();
        }
        req
    }

    /// Odpowiedź odrzucona: poprawka idzie do modelu szczebel mocniejszego niż ten, który ją
    /// napisał (Router: `escalate_from`). Zwraca opis dla notatki weryfikacji.
    pub(crate) fn escalate(&mut self) -> Option<String> {
        // Bez mocniejszego modelu podniesienie tylko powtórzyłoby ten sam model.
        let model = self
            .cp
            .answer_model
            .clone()
            .filter(|m| self.shared.provider.can_escalate(m))?;
        self.cp.route.escalate_from = Some(model.clone());
        Some(format!("poprawka trafi do mocniejszego modelu niż {model}"))
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
        let mut invalid = BTreeMap::new();
        let mut time_limited = false;
        loop {
            let remaining = self
                .cp
                .spec
                .budget
                .max_wall_ms
                .saturating_sub(self.elapsed_ms())
                .max(1);
            let next = tokio::select! {next=stream.next()=>next,()=tokio::time::sleep(std::time::Duration::from_millis(remaining))=>{if self.auto_extend().await {continue;}time_limited=true;break;}};
            let Some(ev) = next else {
                break;
            };
            crate::invalid::note(&ev, &mut invalid);
            acc.push(&ev);
        }
        let turn = acc.finish();
        self.cp.usage.input_tokens += turn.usage.total_input();
        self.cp.usage.output_tokens += turn.usage.output_tokens;
        // Cena modelu, który faktycznie odpowiedział (`Started`/`ModelSwitched`; u Routera
        // `dostawca:model`) — bez przypięcia `spec.model` to „auto” i koszt wychodził 0; potem
        // model przypięty (odpowiedź bywa z nazwą spoza cennika), jak w zwykłej rozmowie.
        let cost = turn
            .model
            .as_deref()
            .and_then(|m| provider.cost(m, &turn.usage))
            .or_else(|| provider.cost(&self.cp.spec.model, &turn.usage));
        let nano = cost.map_or(0, |c| c.nano_usd);
        self.cp.usage.cost_nano_usd = self.cp.usage.cost_nano_usd.saturating_add(nano);
        if let Some(model) = turn.model.as_ref() {
            let p = self.cp.model_usage.entry(model.clone()).or_default();
            p.0 = p.0.saturating_add(turn.usage.total_input());
            p.1 = p.1.saturating_add(turn.usage.output_tokens);
            if let Some(c) = cost {
                p.2 = Some(p.2.unwrap_or(0).saturating_add(c.nano_usd));
            }
        }
        self.handle
            .emit(RunEvent::ModelUsage {
                model: turn.model.clone(),
                input_tokens: turn.usage.total_input(),
                output_tokens: turn.usage.output_tokens,
                cost_nano_usd: cost.map(|c| c.nano_usd),
            })
            .await;
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
        if time_limited {
            let text = turn.message.visible_text();
            if !text.is_empty() {
                self.push(Role::Assistant, vec![ContentBlock::text(text)]);
            }
            self.handle
                .emit(finished(
                    StepStatus::Failed,
                    "Limit czasu aktywnego; częściowy wynik zapisany lokalnie.".into(),
                ))
                .await;
            self.handle
                .emit(RunEvent::BudgetExceeded {
                    budget: agent_runtime_contract::BudgetKind::Wall,
                })
                .await;
            return Next::Finish(RunOutcome::BudgetExceeded {
                budget: agent_runtime_contract::BudgetKind::Wall,
            });
        }
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
                provider: Some(err.clone()),
                model: turn.model.clone(),
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
                    provider: None,
                    model: None,
                });
            }
            _ => {}
        }
        if !self.cp.verifying
            && let Some(model) = turn.model.clone().filter(|m| !m.is_empty())
        {
            self.cp.answer_model = Some(model);
        }
        let text = turn.message.visible_text();
        self.invalid_args = invalid;
        let mut uses = self.normalize_calls(turn.message.tool_uses().cloned().collect());
        let mut message = turn.message.clone();
        // Werdykt kończy weryfikację: wywołania doklejone do odpowiedzi z werdyktem (lekki model
        // w kopercie dopisuje je odruchowo) nie są wykonywane ani zapisywane w historii — inaczej
        // weryfikacja krążyła „OK → fs_read” aż do wykrycia pętli (pomiar F3, 2026-10-29). To samo
        // w podprzebiegu Krytyczki: „WERYFIKACJA: OK → browser_read” do końca budżetu kroków
        // (próba obowiązku „morning”, 1.11).
        let judging = self.cp.verifying || self.critic_run();
        if judging && !uses.is_empty() && verdict(&text).is_some() {
            uses.clear();
            message
                .content
                .retain(|b| !matches!(b, ContentBlock::ToolUse(_)));
        }
        if !message.content.is_empty() {
            self.cp.messages.push(message);
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
            return self
                .final_answer(text, turn.stop == Some(StopReason::MaxTokens))
                .await;
        }
        self.run_tools(uses).await
    }

    /// Odpowiedź końcowa: weryfikacja (Krytyczka ≠ autorka, gdy obsada na to pozwala).
    async fn final_answer(&mut self, text: String, truncated: bool) -> Next {
        // Urwana odpowiedź albo dane ze stron, których przebieg nie odczytał (`completeness`) — nie
        // w podprzebiegu Krytyczki: jej cel zawiera odpowiedź autorki (niezaufaną, z adresami).
        let judging = self.cp.verifying || self.critic_run();
        if !judging
            && crate::completeness::nudges(&self.cp.messages) < crate::completeness::MAX_NUDGES
        {
            let can_read = crate::completeness::can_read(&self.registry.names());
            let gap = crate::completeness::gap(
                &text,
                truncated,
                &self.cp.spec.goal,
                &self.cp.messages,
                can_read,
            );
            if let Some(gap) = gap {
                self.push(
                    Role::User,
                    vec![ContentBlock::text(gap.note(&self.cp.spec.goal))],
                );
                return Next::Continue;
            }
        }
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
                self.escalate();
                self.push(
                    Role::User,
                    vec![ContentBlock::text(format!(
                        "[Weryfikacja] Wykryto problem — popraw wynik, a potem zakończ. {}",
                        crate::completeness::goal_reminder(&self.cp.spec.goal)
                    ))],
                );
                return Next::Continue;
            }
            // Wynikiem jest sprawdzana odpowiedź (jak przy Krytyczce), a nie tekst werdyktu —
            // inaczej użytkownik dostawał „WERYFIKACJA: OK” zamiast odpowiedzi (pomiar F3).
            let answer = self
                .cp
                .verified_answer
                .take()
                .filter(|a| !a.trim().is_empty())
                .unwrap_or(text);
            return Next::Finish(RunOutcome::Completed {
                summary: answer,
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
        self.cp.verified_answer = Some(text);
        let reminder = crate::completeness::goal_reminder(&self.cp.spec.goal);
        self.push(
            Role::User,
            vec![ContentBlock::text(format!("{VERIFY_PROMPT} {reminder}"))],
        );
        Next::Continue
    }
}
