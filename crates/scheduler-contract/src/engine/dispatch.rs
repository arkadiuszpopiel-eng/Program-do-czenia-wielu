//! Przegląd kolejki (`pump`) i przydział: ranga (klasa ↓, termin ↑, kolejność zgłoszeń ↑),
//! okna czasowe, obsada i limity równoległości, budżet tła, atomowe przyznanie kompletu zasobów,
//! rezerwacje dla zablokowanych zadań wyższej rangi i wywłaszczanie niższej klasy w punkcie
//! atomowym.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use cost_meter_contract::BudgetDecision;
use personas_contract::PersonaId;
use scheduler_lite_contract::{Holder, LeaseRequest, Resource, SchedError};
use serde_json::json;

use crate::engine::{Ctx, SchedEffect};
use crate::events::{EVENT_BUDGET_WARNING, EVENT_DISPATCHED, EVENT_STEERED};
use crate::host::SchedHost;
use crate::ids::{DispatchId, TaskId};
use crate::spec::{Assignee, TaskClass, TaskSpec};
use crate::state::{BlockReason, TaskOutput, TaskState, Termination};
use crate::steer::{Dispatch, YieldReason};

/// Górna granica rund przeglądu (każda runda zmienia stan co najmniej jednego zadania).
const MAX_PUMP_ROUNDS: usize = 16_384;

/// Posiadaczka dzierżaw zadania w tablicy blokad.
pub(crate) fn lease_holder(task: &TaskId, agent: Option<&PersonaId>) -> Holder {
    match agent {
        Some(p) => Holder::Persona(p.clone()),
        None => Holder::System(format!("task:{task}")),
    }
}

fn lease_requests(spec: &TaskSpec, holder: &Holder) -> Vec<LeaseRequest> {
    spec.resources
        .iter()
        .map(|r| {
            LeaseRequest::new(
                r.clone(),
                holder.clone(),
                spec.class.lease_priority(),
                Duration::ZERO,
            )
        })
        .collect()
}

/// Liczniki zajętości.
#[derive(Default)]
struct Load {
    by_agent: BTreeMap<PersonaId, u32>,
    agents: u32,
    system: u32,
}

impl<H: SchedHost> Ctx<'_, H> {
    /// Przegląd: terminy, ponowienia, zależności, przydział — do punktu stałego.
    pub(crate) fn pump(&mut self) {
        for _ in 0..MAX_PUMP_ROUNDS {
            let mut progressed = self.enforce_running();
            progressed |= self.expire_waiting();
            progressed |= self.wake_retries();
            progressed |= self.resolve_dependencies();
            progressed |= self.dispatch_ready();
            if !progressed {
                break;
            }
        }
        self.prune();
    }

    fn load(&self) -> Load {
        let mut load = Load::default();
        for rec in self.inner.st.tasks.values() {
            if let TaskState::Running { agent, .. } = &rec.state {
                match agent {
                    Some(p) => {
                        *load.by_agent.entry(p.clone()).or_default() += 1;
                        load.agents += 1;
                    }
                    None => load.system += 1,
                }
            }
        }
        load
    }

    /// Gotowe zadania w kolejności rangi.
    fn ranked_ready(&self) -> Vec<TaskId> {
        let mut ready: Vec<(Reverse<TaskClass>, u64, u64, TaskId)> = self
            .inner
            .st
            .tasks
            .iter()
            .filter(|(_, r)| matches!(r.state, TaskState::Ready))
            .map(|(id, r)| (Reverse(r.spec.class), r.deadline_ms, r.seq, id.clone()))
            .collect();
        ready.sort();
        ready.into_iter().map(|(_, _, _, id)| id).collect()
    }

    /// Przydziela, co się da. Zwraca, czy coś wystartowało albo się zakończyło.
    pub(crate) fn dispatch_ready(&mut self) -> bool {
        let mut load = self.load();
        let mut reserved: BTreeSet<Resource> = BTreeSet::new();
        let mut progressed = false;
        for id in self.ranked_ready() {
            let Some(rec) = self.inner.st.tasks.get(&id) else {
                continue;
            };
            let spec = rec.spec.clone();
            if let Some(reason) = self.window_block(&spec) {
                self.set_blocked(&id, Some(reason));
                continue;
            }
            let roster = &self.inner.st.roster;
            let system = matches!(spec.assignee, Assignee::System(_));
            let full = if system {
                load.system >= roster.max_parallel_system
            } else {
                load.agents >= roster.max_parallel_total
            };
            if full {
                self.set_blocked(&id, Some(BlockReason::Concurrency));
                continue;
            }
            let agent = match roster.pick(&spec.assignee, &load.by_agent) {
                Ok(agent) => agent,
                Err(reason) => {
                    self.set_blocked(&id, Some(reason));
                    continue;
                }
            };
            let taken: Vec<Resource> = spec
                .resources
                .iter()
                .filter(|r| reserved.contains(*r))
                .cloned()
                .collect();
            if !taken.is_empty() {
                self.set_blocked(&id, Some(BlockReason::Reserved { resources: taken }));
                continue;
            }
            if !self.background_budget_ok(&id, &spec) {
                progressed = true;
                continue;
            }
            let holder = lease_holder(&id, agent.as_ref());
            match self.locks.try_acquire_all(&lease_requests(&spec, &holder)) {
                Ok(leases) => {
                    self.inner.leases.insert(id.clone(), leases);
                    self.start(&id, agent.clone());
                    match agent {
                        Some(p) => {
                            *load.by_agent.entry(p).or_default() += 1;
                            load.agents += 1;
                        }
                        None => load.system += 1,
                    }
                    progressed = true;
                }
                Err(SchedError::Timeout { .. }) => {
                    let busy: Vec<Resource> = spec
                        .resources
                        .iter()
                        .filter(|r| !self.locks.all_free_for(&[(*r).clone()], &holder))
                        .cloned()
                        .collect();
                    self.request_preemption(&spec, &busy);
                    reserved.extend(spec.resources.iter().cloned());
                    self.set_blocked(&id, Some(BlockReason::Resources { busy }));
                }
                Err(other) => {
                    let attempts = self.inner.st.tasks.get(&id).map_or(1, |r| r.failures + 1);
                    self.finalize(
                        &id,
                        Termination::Failed {
                            error: format!("zasoby: {other}"),
                            attempts,
                        },
                    );
                    progressed = true;
                }
            }
        }
        progressed
    }

    /// Okno czasowe: „nie wcześniej niż”, bezczynność, tryb gry.
    fn window_block(&self, spec: &TaskSpec) -> Option<BlockReason> {
        let w = &spec.window;
        let c = self.inner.st.conditions;
        if let Some(at_ms) = w.not_before_ms.filter(|t| *t > self.now) {
            return Some(BlockReason::NotBefore { at_ms });
        }
        if w.only_when_idle && !c.user_idle {
            return Some(BlockReason::NotIdle);
        }
        (w.not_in_game_mode && c.game_mode).then_some(BlockReason::GameMode)
    }

    /// Budżet tła (`cost-meter`): blokada kończy zadanie z jawnym powodem.
    fn background_budget_ok(&mut self, id: &TaskId, spec: &TaskSpec) -> bool {
        let estimate = spec.budget.estimated_cost_micro_pln;
        if spec.class != TaskClass::Background || estimate == 0 {
            return true;
        }
        match self.host.check_background_budget(estimate) {
            BudgetDecision::Allow => true,
            BudgetDecision::Warn { notices } => {
                self.event(EVENT_BUDGET_WARNING, id, json!({ "notices": notices }));
                true
            }
            BudgetDecision::Block { notice } => {
                let reason = format!(
                    "budżet tła: wydano {} z {} mikro-PLN, zadanie ~{}",
                    notice.spent_micro_pln, notice.limit_micro_pln, notice.estimate_micro_pln
                );
                self.finalize(id, Termination::BudgetBlocked { reason });
                false
            }
        }
    }

    /// Zadanie wyższej klasy czeka na zasób wywłaszczalny trzymany przez zadanie niższej klasy
    /// → prośba o oddanie w najbliższym punkcie atomowym (bez zabijania).
    fn request_preemption(&mut self, waiting: &TaskSpec, busy: &[Resource]) {
        for resource in busy {
            if !self.locks.policy(resource).preemptible_at_atomic {
                continue;
            }
            let holder = self.inner.leases.iter().find_map(|(task, ls)| {
                ls.iter()
                    .any(|l| l.resource() == resource)
                    .then(|| task.clone())
            });
            let Some(holder) = holder else {
                continue;
            };
            if let Some(rec) = self.inner.st.tasks.get_mut(&holder)
                && rec.spec.class < waiting.class
                && rec.yield_request.is_none()
                && rec.stop_request.is_none()
            {
                rec.yield_request = Some(YieldReason::Preempted {
                    resource: Some(resource.clone()),
                });
                self.out.dirty = true;
            }
        }
    }

    /// Start wykonania: stan `Running`, steering sprzed startu, wyniki poprzedniczek.
    fn start(&mut self, id: &TaskId, agent: Option<PersonaId>) {
        self.inner.st.next_dispatch += 1;
        let dispatch = DispatchId(self.inner.st.next_dispatch);
        let inputs: BTreeMap<TaskId, TaskOutput> = match self.inner.st.tasks.get(id) {
            Some(rec) => rec
                .spec
                .deps
                .iter()
                .filter_map(|d| {
                    let dep = self.inner.st.tasks.get(&d.task)?;
                    match &dep.state {
                        TaskState::Done {
                            termination: Termination::Succeeded { output },
                        } => Some((d.task.clone(), output.clone())),
                        _ => None,
                    }
                })
                .collect(),
            None => return,
        };
        let Some(rec) = self.inner.st.tasks.get_mut(id) else {
            return;
        };
        rec.state = TaskState::Running {
            dispatch,
            agent: agent.clone(),
            since_ms: self.now,
        };
        rec.blocked = None;
        rec.yield_request = None;
        rec.last_dispatch = Some(dispatch);
        let steering = std::mem::take(&mut rec.steering);
        let interrupted = std::mem::take(&mut rec.interrupted);
        let d = Dispatch {
            dispatch,
            task: id.clone(),
            attempt: rec.failures + 1,
            agent,
            resume_from_step: rec.steps,
            interrupted,
            spec: rec.spec.clone(),
            steering,
            inputs,
        };
        let steps = rec.steps;
        self.event(
            EVENT_DISPATCHED,
            id,
            json!({
                "dispatch": dispatch, "attempt": d.attempt, "resources": d.spec.resources,
                "resume_from_step": d.resume_from_step, "interrupted": interrupted,
            }),
        );
        for env in &d.steering {
            let latency = steps.saturating_sub(env.sent_at_step);
            self.event(
                EVENT_STEERED,
                id,
                json!({ "seq": env.seq, "kind": env.steer.kind(), "latency_steps": latency, "at_step": steps }),
            );
        }
        self.out.effects.push(SchedEffect::Dispatch(Box::new(d)));
        self.out.dirty = true;
    }
}
