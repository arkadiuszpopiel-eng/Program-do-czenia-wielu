//! Operacje publiczne rdzenia: zgłoszenie, delegacja, punkty atomowe, wynik, anulowanie
//! poddrzewa, pauza/wznowienie, steering, obsada, warunki, kill-switch.

use serde_json::json;

use crate::engine::{Ctx, SchedCore, SchedEffect};
use crate::error::TaskError;
use crate::events::{EVENT_KILL_SWITCH, EVENT_PAUSED, EVENT_RESUMED, global_event};
use crate::host::SchedHost;
use crate::ids::{DispatchId, TaskId};
use crate::roster::{Roster, SystemConditions};
use crate::spec::{ExecutorKind, TaskSpec};
use crate::state::{BlockReason, CancelCause, TaskState, Termination};
use crate::steer::{
    Steer, SteerEnvelope, StepDirective, StepReport, StopReason, WorkerResult, YieldReason,
};
use crate::validate::MAX_PENDING_STEERS;

impl<H: SchedHost> Ctx<'_, H> {
    fn known(&self, id: &TaskId) -> Result<&crate::engine::TaskRec, TaskError> {
        let rec = self
            .inner
            .st
            .tasks
            .get(id)
            .ok_or_else(|| TaskError::UnknownTask(id.clone()))?;
        if rec.state.is_terminal() {
            return Err(TaskError::AlreadyFinished(id.clone()));
        }
        Ok(rec)
    }

    /// Anulowanie poddrzewa: korzeń i potomkowie; zadania w toku zatrzymują się w punkcie
    /// atomowym (najpóźniej po `STOP_GRACE_MS` — siłą).
    pub(crate) fn cancel(&mut self, root: &TaskId, reason: &str) -> Result<Vec<TaskId>, TaskError> {
        self.known(root)?;
        let mut affected = Vec::new();
        for id in self.subtree(root) {
            let Some(rec) = self.inner.st.tasks.get(&id) else {
                continue;
            };
            if rec.state.is_terminal() {
                continue;
            }
            let cause = if &id == root {
                CancelCause::User {
                    reason: reason.to_owned(),
                }
            } else {
                CancelCause::Ancestor { root: root.clone() }
            };
            if rec.running_dispatch().is_some() {
                self.request_stop(&id, StopReason::Cancelled { cause });
            } else {
                self.finalize(&id, Termination::Cancelled { cause });
            }
            affected.push(id);
        }
        Ok(affected)
    }

    /// Pauza: od razu (czekające) albo w najbliższym punkcie atomowym (w toku).
    pub(crate) fn pause(&mut self, id: &TaskId) -> Result<(), TaskError> {
        let running = self.known(id)?.running_dispatch().is_some();
        let Some(rec) = self.inner.st.tasks.get_mut(id) else {
            return Err(TaskError::UnknownTask(id.clone()));
        };
        if running {
            rec.yield_request = Some(YieldReason::Paused);
        } else if !matches!(rec.state, TaskState::Paused) {
            rec.state = TaskState::Paused;
            rec.blocked = Some(BlockReason::Paused);
            self.event(EVENT_PAUSED, id, json!({}));
        }
        self.out.dirty = true;
        Ok(())
    }

    /// Wznowienie: wstrzymane wraca do rozstrzygania zależności; niezrealizowana prośba
    /// o pauzę jest wycofywana.
    pub(crate) fn resume(&mut self, id: &TaskId) -> Result<(), TaskError> {
        self.known(id)?;
        let Some(rec) = self.inner.st.tasks.get_mut(id) else {
            return Err(TaskError::UnknownTask(id.clone()));
        };
        match rec.state {
            TaskState::Paused => {
                rec.state = TaskState::Pending;
                rec.blocked = Some(BlockReason::Dependencies);
                self.event(EVENT_RESUMED, id, json!({}));
            }
            TaskState::Running { .. } if rec.yield_request == Some(YieldReason::Paused) => {
                rec.yield_request = None;
            }
            _ => {}
        }
        self.out.dirty = true;
        Ok(())
    }

    /// Steering: treść w kolejce zadania (dostarczana w najbliższym punkcie atomowym albo przy
    /// starcie), operacje (pauza, wznowienie, anulowanie) od razu.
    pub(crate) fn steer(&mut self, id: &TaskId, steer: Steer) -> Result<u64, TaskError> {
        match steer {
            Steer::PauseAfterCurrent => return self.pause(id).map(|()| 0),
            Steer::Resume => return self.resume(id).map(|()| 0),
            Steer::Cancel => return self.cancel(id, "steering: anuluj").map(|_| 0),
            Steer::Message { .. } | Steer::ChangeGoal { .. } => {}
        }
        let rec = self.known(id)?;
        if rec.steering.len() >= MAX_PENDING_STEERS {
            return Err(TaskError::InvalidSpec {
                task: id.clone(),
                reason: "za dużo oczekujących wiadomości sterujących".into(),
            });
        }
        self.inner.st.next_steer += 1;
        let seq = self.inner.st.next_steer;
        if let Some(rec) = self.inner.st.tasks.get_mut(id) {
            rec.steering.push(SteerEnvelope {
                seq,
                steer,
                sent_at_ms: self.now,
                sent_at_step: rec.steps,
            });
        }
        self.out.dirty = true;
        Ok(seq)
    }

    /// Kill-switch: wszystkie zadania anulowane, wykonawczynie przerywane od razu, dzierżawy
    /// i czekające żądania mowy odebrane. Zwraca liczbę objętych zadań i dzierżaw/żądań.
    pub(crate) fn kill_all(&mut self) -> usize {
        let active: Vec<(TaskId, Option<DispatchId>)> = self
            .inner
            .st
            .tasks
            .iter()
            .filter(|(_, r)| !r.state.is_terminal())
            .map(|(id, r)| (id.clone(), r.running_dispatch()))
            .collect();
        let reason = StopReason::Cancelled {
            cause: CancelCause::KillSwitch,
        };
        for (id, dispatch) in &active {
            match dispatch {
                Some(d) => self.abort(id, *d, &reason),
                None => self.finalize(
                    id,
                    Termination::Cancelled {
                        cause: CancelCause::KillSwitch,
                    },
                ),
            }
        }
        let leases = self.locks.kill_all();
        self.out.events.push(global_event(
            EVENT_KILL_SWITCH,
            self.now,
            json!({ "cancelled": active.len(), "leases": leases }),
        ));
        self.out.dirty = true;
        active.len() + leases
    }

    /// Nowe warunki: zadania „tylko w bezczynności”/„nie w trybie gry” w toku oddają zasoby
    /// w najbliższym punkcie atomowym.
    pub(crate) fn set_conditions(&mut self, conditions: SystemConditions) {
        self.inner.st.conditions = conditions;
        for rec in self.inner.st.tasks.values_mut() {
            let w = &rec.spec.window;
            let lost = (w.only_when_idle && !conditions.user_idle)
                || (w.not_in_game_mode && conditions.game_mode);
            if lost && rec.running_dispatch().is_some() && rec.yield_request.is_none() {
                rec.yield_request = Some(YieldReason::ConditionLost);
            }
        }
        self.out.dirty = true;
    }

    /// Delegacja z wykonania: rodzic = zadanie w toku; pochodzenie i taint dziedziczone
    /// (nie da się ich „podnieść”), klasa najwyżej jak rodzica.
    pub(crate) fn spawn(
        &mut self,
        dispatch: DispatchId,
        mut specs: Vec<TaskSpec>,
    ) -> Result<Vec<TaskId>, TaskError> {
        let parent_id = self
            .running_task(dispatch)
            .ok_or(TaskError::StaleDispatch(dispatch))?;
        let Some(parent) = self.inner.st.tasks.get(&parent_id) else {
            return Err(TaskError::UnknownTask(parent_id));
        };
        let parent = parent.spec.clone();
        // Przegląd #2 (SR2-08): podzadanie z wykonania to decyzja agentki — nigdy most CLI, także
        // pod zadaniem użytkownika (most startuje tylko z jawnego polecenia właściciela).
        if let Some(bridge) = specs
            .iter()
            .find(|s| matches!(s.executor, ExecutorKind::Bridge(_)))
        {
            return Err(TaskError::BridgeNotAllowed {
                task: bridge.id.clone(),
                origin: "delegacja agentki (StepGate::spawn)".into(),
            });
        }
        for spec in &mut specs {
            spec.parent = Some(parent_id.clone());
            spec.origin = parent.origin.clone();
            for source in &parent.taint {
                if !spec.taint.contains(source) {
                    spec.taint.push(source.clone());
                }
            }
            spec.session = spec.session.take().or_else(|| parent.session.clone());
            spec.class = spec.class.min(parent.class);
        }
        self.submit_batch(specs)
    }
}

impl<H: SchedHost> SchedCore<H> {
    /// Zgłasza zadania (całość albo nic).
    pub fn submit(
        &self,
        specs: Vec<TaskSpec>,
    ) -> Result<(Vec<TaskId>, Vec<SchedEffect>), TaskError> {
        self.op(|ctx| ctx.submit_batch(specs))
    }

    /// Delegacja: podzadania zadania w toku (wywołuje wykonawczyni).
    pub fn spawn(
        &self,
        dispatch: DispatchId,
        specs: Vec<TaskSpec>,
    ) -> Result<(Vec<TaskId>, Vec<SchedEffect>), TaskError> {
        self.op(|ctx| ctx.spawn(dispatch, specs))
    }

    /// Przegląd kolejki (po upływie czasu albo zmianie zasobów).
    pub fn pump(&self) -> Vec<SchedEffect> {
        self.op(|_| Ok(())).map(|(_, fx)| fx).unwrap_or_default()
    }

    /// Punkt atomowy wykonawczyni.
    pub fn boundary(
        &self,
        dispatch: DispatchId,
        report: &StepReport,
    ) -> (StepDirective, Vec<SchedEffect>) {
        match self.op(|ctx| Ok(ctx.boundary(dispatch, report))) {
            Ok(r) => r,
            Err(_) => (
                StepDirective::Stop {
                    reason: StopReason::Cancelled {
                        cause: CancelCause::Worker,
                    },
                },
                Vec::new(),
            ),
        }
    }

    /// Wynik wykonawczyni.
    pub fn finish(&self, dispatch: DispatchId, result: WorkerResult) -> Vec<SchedEffect> {
        self.op(|ctx| {
            ctx.finish(dispatch, result);
            Ok(())
        })
        .map(|(_, fx)| fx)
        .unwrap_or_default()
    }

    /// Anuluje zadanie i jego poddrzewo; zwraca objęte zadania.
    pub fn cancel(
        &self,
        task: &TaskId,
        reason: &str,
    ) -> Result<(Vec<TaskId>, Vec<SchedEffect>), TaskError> {
        self.op(|ctx| ctx.cancel(task, reason))
    }

    /// Wstrzymuje zadanie.
    pub fn pause(&self, task: &TaskId) -> Result<Vec<SchedEffect>, TaskError> {
        self.op(|ctx| ctx.pause(task)).map(|(_, fx)| fx)
    }

    /// Wznawia zadanie.
    pub fn resume(&self, task: &TaskId) -> Result<Vec<SchedEffect>, TaskError> {
        self.op(|ctx| ctx.resume(task)).map(|(_, fx)| fx)
    }

    /// Steering; zwraca numer wiadomości (0 dla operacji pauza/wznowienie/anulowanie).
    pub fn steer(&self, task: &TaskId, steer: Steer) -> Result<(u64, Vec<SchedEffect>), TaskError> {
        self.op(|ctx| ctx.steer(task, steer))
    }

    /// Nowa obsada.
    pub fn set_roster(&self, roster: Roster) -> Vec<SchedEffect> {
        self.op(|ctx| {
            ctx.inner.st.roster = roster;
            ctx.out.dirty = true;
            Ok(())
        })
        .map(|(_, fx)| fx)
        .unwrap_or_default()
    }

    /// Nowe warunki systemowe.
    pub fn set_conditions(&self, conditions: SystemConditions) -> Vec<SchedEffect> {
        self.op(|ctx| {
            ctx.set_conditions(conditions);
            Ok(())
        })
        .map(|(_, fx)| fx)
        .unwrap_or_default()
    }

    /// Kill-switch: zadania i dzierżawy. Zwraca liczbę objętych zadań i dzierżaw/żądań.
    pub fn kill_all(&self) -> (usize, Vec<SchedEffect>) {
        self.op(|ctx| Ok(ctx.kill_all())).unwrap_or((0, Vec::new()))
    }
}
