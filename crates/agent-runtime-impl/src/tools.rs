//! Wykonanie narzędzi tury: granica kroku przed każdym wywołaniem, kolejne wywołania tylko do
//! odczytu równolegle (jedna paczka), zapisy i delegacja szeregowo z dzierżawami zasobów,
//! obserwacja (taint, proweniencja), zdarzenia kroków.

use std::sync::Arc;

use agent_runtime_contract::{RunEvent, RunOutcome, StepKind, StepStatus};
use futures_util::future::join_all;
use providers_contract::{ContentBlock, Role, ToolUse};
use safety_broker_contract::{ApprovalId, ApprovalTicket, Holder};
use tools_common_contract::{Tool, ToolCtx, ToolErrorKind, ToolObserver, ToolOutcome, ToolStatus};

use crate::boundary::{Boundary, SKIP_YIELDED, fnv};
use crate::engine::{Engine, Next};
use crate::flow::{append_capped, args_untrusted, fingerprint, repeats};
use crate::handle::RunHandle;
use crate::prompt::{skipped, summary, tool_result};

/// Rodziny zdolności utrwalające treść poza przebiegiem (pamięć długoterminowa).
const PERSISTENT_WRITE_CAPS: [&str; 1] = ["memory.write"];

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

/// Wywołanie przygotowane (krok rozpoczęty).
struct Prepared {
    index: usize,
    step: u32,
    input: String,
    tool: Option<Arc<dyn Tool>>,
    ctx: ToolCtx,
}

fn stop_reason(next: &Next) -> &'static str {
    match next {
        Next::Yield => SKIP_YIELDED,
        Next::Finish(RunOutcome::Cancelled) => "anulowano",
        _ => "przebieg zatrzymany",
    }
}

impl Engine {
    pub(crate) fn ctx(&self, step: u32, args: &serde_json::Value, title: &str) -> ToolCtx {
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
    async fn gate_call(&mut self, index: usize, tu: &ToolUse) -> Option<(String, Option<Next>)> {
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
        if let Some(budget) = self.exceeded() {
            self.handle.emit(RunEvent::BudgetExceeded { budget }).await;
            return Some((
                format!("przekroczony budżet {}", budget.label_pl()),
                Some(Next::Finish(RunOutcome::BudgetExceeded { budget })),
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
        None
    }

    /// Rozpoczyna krok narzędzia (numer, zdarzenie, kontekst).
    pub(crate) async fn begin_call(&mut self, tu: &ToolUse) -> (u32, String) {
        self.cp.usage.steps += 1;
        self.cp.usage.tool_calls += 1;
        let step = self.cp.usage.steps;
        let input = summary(&tu.input.to_string(), 200);
        self.handle
            .emit(RunEvent::StepStarted {
                step,
                kind: StepKind::Tool,
                tool: Some(tu.name.clone()),
                input: input.clone(),
            })
            .await;
        (step, input)
    }

    async fn prepare_call(&mut self, index: usize, tu: &ToolUse) -> Prepared {
        let (step, input) = self.begin_call(tu).await;
        let tool = self.registry.get(&tu.name).cloned();
        let title = tool
            .as_ref()
            .map_or_else(|| tu.name.clone(), |t| t.manifest().title.clone());
        let mut ctx = self.ctx(step, &tu.input, &title);
        // Przegląd #2 (SR2-07): treść utrwalana poza przebiegiem (pamięć) w przebiegu skażonym
        // pochodzi z kontekstu z niezaufaną treścią — proweniencja „niezaufana” (S19: taki wpis
        // zostaje w sesji i nigdy nie awansuje), niezależnie od heurystyki argumentów-celów.
        if self.cp.tainted
            && tool.as_ref().is_some_and(|t| {
                t.manifest()
                    .capabilities
                    .iter()
                    .any(|c| PERSISTENT_WRITE_CAPS.contains(&c.as_str()))
            })
        {
            ctx.untrusted_args = true;
        }
        Prepared {
            index,
            step,
            input,
            tool,
            ctx,
        }
    }

    fn unknown(&self, name: &str) -> ToolOutcome {
        ToolOutcome::failed(
            ToolErrorKind::InvalidArgs,
            format!(
                "Nieznane albo niedostępne narzędzie `{name}`. Dostępne: {}.",
                self.registry.names().join(", ")
            ),
        )
    }

    /// Obserwacja: taint, proweniencja, zdarzenie końca kroku, wynik dla modelu.
    pub(crate) async fn observe(
        &mut self,
        tu: &ToolUse,
        step: u32,
        input: String,
        outcome: &ToolOutcome,
    ) -> ContentBlock {
        if let Some(source) = outcome.untrusted.clone() {
            if !self.cp.tainted {
                self.cp.tainted = true;
                self.cp.taint_source = Some(source.clone());
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
        let block_id = format!("{step}-{}", tu.id);
        let max = self.shared.config.tool_result_max_chars;
        ContentBlock::ToolResult(tool_result(&tu.id, &tu.name, outcome, &block_id, max))
    }

    /// Jedno wywołanie szeregowo (zapis z dzierżawami albo delegacja).
    async fn call_one(&mut self, index: usize, tu: &ToolUse) -> (ContentBlock, ToolOutcome) {
        if self.registry.is_delegate(&tu.name) {
            let (step, input) = self.begin_call(tu).await;
            let outcome = self.delegate(tu, step).await;
            let block = self.observe(tu, step, input, &outcome).await;
            return (block, outcome);
        }
        let manifest = self.registry.manifest(&tu.name).cloned();
        let leases = match &manifest {
            Some(m) => match self.acquire(m, &tu.input).await {
                Ok(l) => l,
                Err(why) => {
                    let o = ToolOutcome::failed(ToolErrorKind::Io, format!("Nie wykonano: {why}."));
                    return (ContentBlock::ToolResult(skipped(&tu.id, &why)), o);
                }
            },
            None => Vec::new(),
        };
        let p = self.prepare_call(index, tu).await;
        let outcome = match &p.tool {
            Some(t) => t.call(tu.input.clone(), &p.ctx).await,
            None => self.unknown(&tu.name),
        };
        drop(leases);
        let block = self.observe(tu, p.step, p.input, &outcome).await;
        (block, outcome)
    }

    /// Paczka wywołań tylko do odczytu — równolegle (jeden punkt atomowy).
    async fn call_parallel(
        &mut self,
        batch: &[(usize, ToolUse)],
    ) -> Vec<(usize, ContentBlock, ToolOutcome)> {
        let mut prepared = Vec::with_capacity(batch.len());
        for (i, tu) in batch {
            prepared.push(self.prepare_call(*i, tu).await);
        }
        let calls = prepared.iter().zip(batch).map(|(p, (_, tu))| {
            let tool = p.tool.clone();
            let ctx = p.ctx.clone();
            let args = tu.input.clone();
            async move {
                match tool {
                    Some(t) => Some(t.call(args, &ctx).await),
                    None => None,
                }
            }
        });
        let outcomes = join_all(calls).await;
        let mut out = Vec::with_capacity(batch.len());
        for ((p, (_, tu)), o) in prepared.into_iter().zip(batch).zip(outcomes) {
            let outcome = o.unwrap_or_else(|| self.unknown(&tu.name));
            let block = self.observe(tu, p.step, p.input, &outcome).await;
            out.push((p.index, block, outcome));
        }
        out
    }

    /// Narzędzia tury; checkpoint przed akcjami (wywołania w toku).
    pub(crate) async fn run_tools(&mut self, uses: Vec<ToolUse>) -> Next {
        self.cp.pending = uses.clone();
        self.checkpoint().await;
        let mut results: Vec<Option<ContentBlock>> = vec![None; uses.len()];
        let mut stop: Option<Next> = None;
        let mut skip: Option<String> = None;
        let max_par = self.shared.config.max_parallel_reads.max(1);
        let mut i = 0;
        while i < uses.len() {
            if let Some(why) = &skip {
                results[i] = Some(ContentBlock::ToolResult(skipped(&uses[i].id, why)));
                i += 1;
                continue;
            }
            match self.tool_boundary().await {
                Boundary::Go => {}
                Boundary::SkipRest(why) => {
                    skip = Some(why.to_owned());
                    continue;
                }
                Boundary::Stop(o) => {
                    let next = Next::Finish(o);
                    skip = Some(stop_reason(&next).to_owned());
                    stop = Some(next);
                    continue;
                }
                Boundary::Yield => {
                    skip = Some(SKIP_YIELDED.to_owned());
                    stop = Some(Next::Yield);
                    continue;
                }
            }
            let mut end = i + 1;
            if self.registry.is_parallel_read(&uses[i].name) {
                while end < uses.len()
                    && end - i < max_par
                    && self.registry.is_parallel_read(&uses[end].name)
                {
                    end += 1;
                }
            }
            let mut ready = Vec::new();
            for (k, tu) in uses.iter().enumerate().take(end).skip(i) {
                if stop.is_some() {
                    results[k] = Some(ContentBlock::ToolResult(skipped(
                        &tu.id,
                        "przebieg zatrzymany",
                    )));
                    continue;
                }
                match self.gate_call(k, tu).await {
                    Some((why, next)) => {
                        results[k] = Some(ContentBlock::ToolResult(skipped(&tu.id, &why)));
                        if let Some(next) = next {
                            skip = Some(stop_reason(&next).to_owned());
                            stop = Some(next);
                        }
                    }
                    None => ready.push((k, tu.clone())),
                }
            }
            let done = if ready.len() > 1 {
                self.call_parallel(&ready).await
            } else if let Some((k, tu)) = ready.first() {
                let (block, outcome) = self.call_one(*k, tu).await;
                vec![(*k, block, outcome)]
            } else {
                Vec::new()
            };
            for (k, block, outcome) in done {
                results[k] = Some(block);
                if outcome.status == ToolStatus::Cancelled && self.handle.cancel.is_cancelled() {
                    skip = Some("anulowano".to_owned());
                    stop = Some(Next::Finish(RunOutcome::Cancelled));
                }
            }
            if std::mem::take(&mut self.yield_requested) && stop.is_none() {
                skip = Some(SKIP_YIELDED.to_owned());
                stop = Some(Next::Yield);
            }
            if self.gate_stopped && stop.is_none() {
                skip = Some("zadanie zatrzymane przez scheduler".to_owned());
                stop = Some(Next::Finish(RunOutcome::Cancelled));
            }
            i = end;
        }
        let blocks = results
            .into_iter()
            .zip(&uses)
            .map(|(r, tu)| {
                r.unwrap_or_else(|| ContentBlock::ToolResult(skipped(&tu.id, "pominięto")))
            })
            .collect();
        self.push(Role::User, blocks);
        self.cp.pending.clear();
        stop.unwrap_or(Next::Continue)
    }
}
