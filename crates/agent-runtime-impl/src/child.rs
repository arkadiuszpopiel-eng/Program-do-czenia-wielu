//! Podprzebiegi z pętli rodzica: start na tym samym runtime (token anulowania potomny, ta sama
//! granica schedulera, sterowanie przekazywane do aktywnego potomka), zbieranie wyniku, taintu
//! i zużycia (liczonego do budżetu rodzica); narzędzie delegacji.

use agent_runtime_contract::{
    Checkpoint, DelegateArgs, RunEvent, RunId, RunOptions, RunOutcome, RunSpec, UsageTotals,
};
use providers_contract::ToolUse;
use safety_broker_contract::TaintSource;
use tools_common_contract::{DenialReason, ToolErrorKind, ToolOutcome};

use crate::delegate::{ParentView, delegation_target, plan_delegation, remaining_budget};
use crate::engine::Engine;
use crate::prompt::summary;
use crate::shared::{Exit, Hooks};

/// Wynik podprzebiegu.
pub(crate) struct ChildResult {
    pub(crate) run: RunId,
    pub(crate) outcome: Option<RunOutcome>,
    pub(crate) taint: Option<TaintSource>,
    pub(crate) usage: UsageTotals,
    /// Scheduler zażądał oddania zadania w trakcie podprzebiegu (potomek przerwany).
    pub(crate) yielded: bool,
}

fn add(acc: &mut UsageTotals, u: &UsageTotals) {
    acc.input_tokens = acc.input_tokens.saturating_add(u.input_tokens);
    acc.output_tokens = acc.output_tokens.saturating_add(u.output_tokens);
    acc.cost_nano_usd = acc.cost_nano_usd.saturating_add(u.cost_nano_usd);
    acc.steps = acc.steps.saturating_add(u.steps);
    acc.tool_calls = acc.tool_calls.saturating_add(u.tool_calls);
}

impl Engine {
    /// Reszta budżetu przebiegu.
    pub(crate) fn remaining(&self) -> agent_runtime_contract::RunBudget {
        let mut used = self.cp.usage;
        add(&mut used, &self.cp.delegated);
        remaining_budget(&self.cp.spec.budget, &used, self.elapsed_ms())
    }

    /// Widok rodzica do planu delegacji / Krytyczki.
    pub(crate) fn parent_view(&self) -> ParentView<'_> {
        let autonomy = match &self.shared.ext.autonomy {
            Some(o) => (
                Some(o.level(&self.cp.spec.session, &self.cp.spec.agent)),
                None,
            ),
            None => (None, None),
        };
        ParentView {
            run: &self.handle.run,
            spec: &self.cp.spec,
            options: &self.cp.options,
            tools: self
                .registry
                .envelope()
                .map(|t| t.manifest().clone())
                .collect(),
            remaining: self.remaining(),
            taint: self.cp.taint_source.clone(),
            autonomy,
            provenance: (&self.cp.trusted_text, &self.cp.untrusted_text),
        }
    }

    /// Uruchamia podprzebieg i czeka na jego koniec; zużycie dolicza do budżetu rodzica.
    pub(crate) async fn run_child(
        &mut self,
        spec: RunSpec,
        options: RunOptions,
    ) -> Result<ChildResult, String> {
        let n = self.handle.children().len() + 1;
        let run = RunId::new(format!("{}-c{n}", self.handle.run));
        let cp = Checkpoint::with_options(run.clone(), spec, options);
        let hooks = Hooks {
            gate: self.hooks.gate.clone(),
            lease_holder: self.hooks.lease_holder.clone(),
            usd_pln_e4: self.hooks.usd_pln_e4,
            initial_steering: Vec::new(),
        };
        let (child, join) = self
            .shared
            .launch(cp, hooks, Some(self.handle.cancel.child_token()))
            .map_err(|e| e.to_string())?;
        self.handle.add_child(&child);
        let exit = join.await.unwrap_or(Exit::Finished(RunOutcome::Failed {
            error: "podprzebieg przerwany".into(),
        }));
        self.handle.clear_active_child();
        let yielded = exit == Exit::Yielded;
        if let Exit::Stopped(_) = exit {
            // Zatrzymanie zadania przez scheduler obejmuje też rodzica.
            self.gate_stopped = true;
        }
        if yielded {
            child.abandon();
            if let Ok(Some(mut c)) = self.shared.store.latest(&run) {
                c.finished = Some(RunOutcome::Cancelled);
                let _ = self.shared.store.save(&c);
            }
        }
        let saved = self.shared.store.latest(&run).ok().flatten();
        let mut usage = UsageTotals::default();
        let mut taint = None;
        match &saved {
            Some(c) => {
                add(&mut usage, &c.usage);
                add(&mut usage, &c.delegated);
                taint.clone_from(&c.taint_source);
            }
            None => {
                for e in child.events() {
                    match e.event {
                        RunEvent::Usage(u) => usage = u,
                        RunEvent::Tainted { source } => taint = taint.or(Some(source)),
                        _ => {}
                    }
                }
            }
        }
        add(&mut self.cp.delegated, &usage);
        Ok(ChildResult {
            run,
            outcome: child.outcome(),
            taint,
            usage,
            yielded,
        })
    }

    /// Narzędzie delegacji: plan (potomek ≤ rodzic) → podprzebieg → wynik dla modelu.
    pub(crate) async fn delegate(&mut self, tu: &ToolUse, _step: u32) -> ToolOutcome {
        let args: DelegateArgs = match serde_json::from_value(tu.input.clone()) {
            Ok(a) => a,
            Err(e) => {
                return ToolOutcome::failed(
                    ToolErrorKind::InvalidArgs,
                    format!("Niepoprawne argumenty delegacji: {e}."),
                );
            }
        };
        let plan = {
            let mut view = self.parent_view();
            if let (Some(oracle), Some(crew)) = (&self.shared.ext.autonomy, &self.cp.options.crew) {
                // Ta sama wykonawczyni co w planie (SR2-01: wcześniej identyfikator bez
                // przycięcia trafiał do Brokera jako nieznana agentka z poziomem domyślnym).
                let target = delegation_target(crew, &args, self.cp.spec.agent.as_str());
                view.autonomy.1 = target.map(|p| {
                    oracle.level(
                        &self.cp.spec.session,
                        &core_bus_contract::AgentId::new(p.as_str()),
                    )
                });
            }
            plan_delegation(&view, &args, self.shared.config.max_delegation_depth)
        };
        let plan = match plan {
            Ok(p) => p,
            Err(e) => {
                return ToolOutcome::denied(DenialReason::Policy, &format!("delegacja ({e})"));
            }
        };
        let who = format!("{} ({})", plan.spec.persona.name, args.role);
        let result = match self.run_child(plan.spec, plan.options).await {
            Ok(r) => r,
            Err(e) => {
                return ToolOutcome::failed(
                    ToolErrorKind::Internal,
                    format!("Delegacja nieudana: {e}."),
                );
            }
        };
        if result.yielded {
            self.yield_requested = true;
        }
        let (ok, text) = match &result.outcome {
            Some(RunOutcome::Completed { summary: s, .. }) => (true, summary(s, 4000)),
            Some(other) => (
                false,
                format!("podzadanie nie skończyło się sukcesem: {other:?}"),
            ),
            None => (false, "podzadanie bez wyniku".to_owned()),
        };
        let body = format!("Wynik od {who}, podprzebieg {}: {text}", result.run);
        let data =
            serde_json::json!({ "run": result.run, "ok": ok, "tokens": result.usage.tokens() });
        let mut outcome = if ok {
            ToolOutcome::ok(body, data)
        } else {
            let mut o = ToolOutcome::failed(ToolErrorKind::Internal, body);
            o.data = data;
            o
        };
        if let Some(source) = result.taint {
            outcome = outcome.untrusted(source);
        }
        outcome
    }
}
