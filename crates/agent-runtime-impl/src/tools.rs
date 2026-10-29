//! Wykonanie narzędzi tury: granica kroku przed każdym wywołaniem, kolejne wywołania tylko do
//! odczytu równolegle (jedna paczka), zapisy i delegacja szeregowo z dzierżawami zasobów,

use std::sync::Arc;

use agent_runtime_contract::{RunEvent, RunOutcome, StepKind};
use futures_util::future::join_all;
use providers_contract::{ContentBlock, Role, ToolUse};
use safety_broker_contract::Holder;
use tools_common_contract::{Tool, ToolCtx, ToolErrorKind, ToolOutcome, ToolStatus};

use crate::boundary::{Boundary, SKIP_YIELDED};
use crate::engine::{Engine, Next};
use crate::flow::{append_capped, args_untrusted, fingerprint};
use crate::prompt::{REPEAT_NOTE, skipped, summary, tool_result};

/// Rodziny zdolności utrwalające treść poza przebiegiem (pamięć długoterminowa).
const PERSISTENT_WRITE_CAPS: [&str; 1] = ["memory.write"];

use crate::approval_observer::{StepObserver, step_status};

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
        ctx.auto_extend_approval = self.cp.options.continuation.auto_approval;
        ctx.observer = Some(Arc::new(StepObserver {
            handle: self.handle.clone(),
            step,
            clock: self.approval_clock.clone(),
        }));
        ctx
    }

    /// Rozpoczyna krok narzędzia (numer, zdarzenie, kontekst).
    pub(crate) async fn begin_call(&mut self, tu: &ToolUse) -> (u32, String) {
        self.cp.usage.steps += 1;
        self.cp.usage.tool_calls += 1;
        let step = self.cp.usage.steps;
        self.cp.started_pending.insert(tu.id.clone());
        self.checkpoint().await;
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

    /// Obserwacja: taint, proweniencja, zdarzenie końca kroku, wynik dla modelu.
    pub(crate) async fn observe(
        &mut self,
        tu: &ToolUse,
        step: u32,
        input: String,
        outcome: &ToolOutcome,
    ) -> ContentBlock {
        self.note_snapshot_failure(tu, outcome);
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
        let mut result = tool_result(&tu.id, &tu.name, outcome, &block_id, max);
        if crate::flow::repeats(&self.cp.recent_calls, &fingerprint(&tu.name, &tu.input)) >= 2 {
            result
                .content
                .push(providers_contract::ToolResultPart::Text {
                    text: REPEAT_NOTE.into(),
                });
        }
        self.cp
            .completed_pending
            .insert(tu.id.clone(), result.clone());
        self.checkpoint().await;
        ContentBlock::ToolResult(result)
    }

    /// Jedno wywołanie szeregowo (zapis z dzierżawami albo delegacja).
    async fn call_one(&mut self, index: usize, tu: &ToolUse) -> (ContentBlock, ToolOutcome) {
        if tu.name == crate::context::RECALL && !self.registry.names().is_empty() {
            let (step, input) = self.begin_call(tu).await;
            let outcome = crate::context::recall(&self.cp.messages, &tu.input);
            let block = self.observe(tu, step, input, &outcome).await;
            return (block, outcome);
        }
        if tu.name == crate::catalog::FIND {
            let (step, input) = self.begin_call(tu).await;
            let outcome = self.find_tools(&tu.input);
            let block = self.observe(tu, step, input, &outcome).await;
            return (block, outcome);
        }
        if self.registry.is_delegate(&tu.name) {
            let (step, input) = self.begin_call(tu).await;
            if let Some(error) = &self.storage_error {
                let outcome = ToolOutcome::failed(
                    ToolErrorKind::Io,
                    format!("Nie zapisano postępu: {error}"),
                );
                return (
                    ContentBlock::ToolResult(skipped(&tu.id, &outcome.text)),
                    outcome,
                );
            }
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
        if let Some(error) = &self.storage_error {
            let outcome =
                ToolOutcome::failed(ToolErrorKind::Io, format!("Nie zapisano postępu: {error}"));
            return (
                ContentBlock::ToolResult(skipped(&tu.id, &outcome.text)),
                outcome,
            );
        }
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
                    && end - i
                        < max_par.min(
                            self.cp
                                .spec
                                .budget
                                .max_steps
                                .saturating_sub(self.cp.usage.steps)
                                .max(1) as usize,
                        )
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
                if matches!(
                    outcome.status,
                    ToolStatus::Denied {
                        reason: tools_common_contract::DenialReason::Policy
                    }
                ) {
                    self.cp
                        .denied_calls
                        .insert(fingerprint(&uses[k].name, &uses[k].input));
                }
                if matches!(outcome.status, ToolStatus::Denied { .. })
                    && !matches!(
                        outcome.status,
                        ToolStatus::Denied {
                            reason: tools_common_contract::DenialReason::Policy
                                | tools_common_contract::DenialReason::ApprovalTimeout { .. }
                                | tools_common_contract::DenialReason::ApprovalExpired { .. }
                        }
                    )
                {
                    skip = Some(
                        "Odmowa uprawnień lub właściciela. Nie ponawiaj przez inny model.".into(),
                    );
                    stop = Some(Next::Finish(RunOutcome::Refused));
                }
                results[k] = Some(block);
                if matches!(
                    outcome.status,
                    ToolStatus::Denied {
                        reason: tools_common_contract::DenialReason::ApprovalTimeout { .. }
                            | tools_common_contract::DenialReason::ApprovalExpired { .. }
                    }
                ) {
                    skip = Some("oczekiwanie na zgodę zakończyło limit; postęp zapisany".into());
                    stop = Some(Next::Finish(RunOutcome::BudgetExceeded {
                        budget: agent_runtime_contract::BudgetKind::ApprovalWait,
                    }));
                }
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
        self.cp.started_pending.clear();
        self.cp.completed_pending.clear();
        stop.unwrap_or(Next::Continue)
    }
}
