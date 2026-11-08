//! Wykonanie: punkty atomowe (budżety, steering, wywłaszczenie, pauza), wynik wykonawczyni,
//! ponowienia z odstępem, terminy i wymuszone przerwanie po czasie na punkt atomowy.

use serde_json::{Value, json};

use crate::engine::{Ctx, SchedEffect};
use crate::events::{
    EVENT_FINISHED, EVENT_LOOP, EVENT_PAUSED, EVENT_RETRY, EVENT_STEER_UNCONSUMED, EVENT_STEERED,
    EVENT_STEP, EVENT_YIELDED, task_event,
};
use crate::host::SchedHost;
use crate::ids::{DispatchId, TaskId};
use crate::state::{BlockReason, BudgetKind, CancelCause, ExpiryReason, TaskState, Termination};
use crate::steer::{StepDirective, StepReport, StopReason, WorkerResult, YieldReason};
use crate::validate::{LOOP_REPEATS, MAX_OUTPUT_BYTES, output_size};

/// Zakończenie wynikające z powodu zatrzymania.
pub(crate) fn termination_for(reason: &StopReason) -> Termination {
    match reason {
        StopReason::Cancelled { cause } => Termination::Cancelled {
            cause: cause.clone(),
        },
        StopReason::Budget { budget } => Termination::BudgetExceeded { budget: *budget },
        StopReason::Deadline => Termination::Expired {
            reason: ExpiryReason::WhileRunning,
        },
    }
}

/// Powód zatrzymania odpowiadający zakończeniu (odpowiedź na spóźniony punkt atomowy).
fn stop_for(termination: Option<&Termination>) -> StopReason {
    match termination {
        Some(Termination::Cancelled { cause }) => StopReason::Cancelled {
            cause: cause.clone(),
        },
        Some(Termination::BudgetExceeded { budget }) => StopReason::Budget { budget: *budget },
        Some(Termination::Expired { .. }) => StopReason::Deadline,
        _ => StopReason::Cancelled {
            cause: CancelCause::Worker,
        },
    }
}

/// Ładunek zdarzenia zakończenia (bez wyniku — może zawierać dane prywatne).
fn finished_payload(termination: &Termination) -> Value {
    let mut p = json!({ "result": termination.kind() });
    match termination {
        Termination::Succeeded { .. } => {}
        Termination::Failed { error, attempts } => {
            p["error"] = json!(error.chars().take(200).collect::<String>());
            p["attempts"] = json!(attempts);
        }
        Termination::Cancelled { cause } => p["cause"] = json!(cause),
        Termination::Skipped {
            dependency,
            condition,
        } => {
            p["dependency"] = json!(dependency);
            p["condition"] = json!(condition);
        }
        Termination::Expired { reason } => p["reason"] = json!(reason),
        Termination::BudgetExceeded { budget } => p["budget"] = json!(budget),
        Termination::BudgetBlocked { reason } => p["reason"] = json!(reason),
    }
    p
}

impl<H: SchedHost> Ctx<'_, H> {
    /// Zdarzenie zadania (agentka z bieżącego wykonania).
    pub(crate) fn event(&mut self, name: &str, id: &TaskId, payload: Value) {
        let agent = self.inner.st.tasks.get(id).and_then(|r| r.agent_label());
        self.out
            .events
            .push(task_event(name, id, agent.as_deref(), self.now, payload));
    }

    /// Zadanie w toku dla wysłania.
    pub(crate) fn running_task(&self, dispatch: DispatchId) -> Option<TaskId> {
        self.inner
            .st
            .tasks
            .iter()
            .find(|(_, r)| r.running_dispatch() == Some(dispatch))
            .map(|(id, _)| id.clone())
    }

    /// Kończy zadanie: zwalnia zasoby, zapisuje powód, zdarzenie i efekt dla czekających.
    pub(crate) fn finalize(&mut self, id: &TaskId, termination: Termination) {
        let mut payload = finished_payload(&termination);
        if let Some(rec) = self.inner.st.tasks.get(id) {
            // Pochodzenie i taint — dla wyzwalaczy „koniec zadania” (łańcuchy, taint) i Marszałka.
            payload["origin"] = json!(rec.spec.origin);
            payload["taint"] = json!(rec.spec.taint);
            payload["class"] = json!(rec.spec.class);
        }
        self.event(EVENT_FINISHED, id, payload);
        let unconsumed: Vec<u64> = self
            .inner
            .st
            .tasks
            .get(id)
            .map(|r| r.steering.iter().map(|e| e.seq).collect())
            .unwrap_or_default();
        if !unconsumed.is_empty() {
            self.event(EVENT_STEER_UNCONSUMED, id, json!({ "seqs": unconsumed }));
        }
        self.close_running(id);
        drop(self.inner.leases.remove(id));
        if let Some(rec) = self.inner.st.tasks.get_mut(id) {
            rec.state = TaskState::Done {
                termination: termination.clone(),
            };
            rec.finished_at_ms = Some(self.now);
            rec.yield_request = None;
            rec.stop_request = None;
            rec.stop_requested_at_ms = None;
        }
        self.out.effects.push(SchedEffect::Finished {
            task: id.clone(),
            termination,
        });
        self.out.dirty = true;
    }

    /// Dolicza czas bieżącego wykonania (przed opuszczeniem stanu `Running`).
    fn close_running(&mut self, id: &TaskId) {
        if let Some(rec) = self.inner.st.tasks.get_mut(id)
            && let TaskState::Running { since_ms, .. } = rec.state
        {
            rec.wall_ms = rec
                .wall_ms
                .saturating_add(self.now.saturating_sub(since_ms));
        }
    }

    /// Prośba o zatrzymanie w najbliższym punkcie atomowym (po `STOP_GRACE_MS` — siłą).
    pub(crate) fn request_stop(&mut self, id: &TaskId, reason: StopReason) {
        if let Some(rec) = self.inner.st.tasks.get_mut(id)
            && rec.stop_request.is_none()
        {
            rec.stop_request = Some(reason);
            rec.stop_requested_at_ms = Some(self.now);
            self.out.dirty = true;
        }
    }

    /// Oddanie zadania: zasoby wracają, zadanie do kolejki (albo pauzy).
    fn do_yield(&mut self, id: &TaskId, reason: &YieldReason) {
        self.event(EVENT_YIELDED, id, json!({ "reason": reason }));
        self.close_running(id);
        drop(self.inner.leases.remove(id));
        let paused = *reason == YieldReason::Paused;
        if let Some(rec) = self.inner.st.tasks.get_mut(id) {
            rec.preemptions = rec.preemptions.saturating_add(1);
            rec.yield_request = None;
            rec.state = if paused {
                TaskState::Paused
            } else {
                TaskState::Ready
            };
            rec.blocked = paused.then_some(BlockReason::Paused);
        }
        if paused {
            self.event(EVENT_PAUSED, id, json!({}));
        }
        self.out.dirty = true;
    }

    /// Punkt atomowy: wykonawczyni ukończyła krok i chce zacząć następny.
    pub(crate) fn boundary(&mut self, dispatch: DispatchId, report: &StepReport) -> StepDirective {
        let Some(id) = self.running_task(dispatch) else {
            let done = self
                .inner
                .st
                .tasks
                .values()
                .find(|r| r.last_dispatch == Some(dispatch))
                .and_then(|r| r.state.termination());
            return StepDirective::Stop {
                reason: stop_for(done),
            };
        };
        let directive = self.decide_at_boundary(&id, report);
        self.out.dirty = true;
        directive
    }

    fn decide_at_boundary(&mut self, id: &TaskId, report: &StepReport) -> StepDirective {
        let (steps, loop_hit) = {
            let Some(rec) = self.inner.st.tasks.get_mut(id) else {
                return StepDirective::Stop {
                    reason: stop_for(None),
                };
            };
            rec.steps = rec.steps.saturating_add(1);
            rec.cost_micro_pln = rec.cost_micro_pln.saturating_add(report.cost_micro_pln);
            match report.fingerprint {
                Some(fp) if rec.last_fingerprint == Some(fp) => rec.repeats += 1,
                Some(_) => rec.repeats = 1,
                None => rec.repeats = 0,
            }
            rec.last_fingerprint = report.fingerprint;
            (rec.steps, rec.repeats == LOOP_REPEATS)
        };
        self.event(
            EVENT_STEP,
            id,
            json!({ "step": steps, "cost_micro_pln": report.cost_micro_pln }),
        );
        if loop_hit {
            self.event(EVENT_LOOP, id, json!({ "repeats": LOOP_REPEATS }));
        }
        if let Some(reason) = self.stop_reason_now(id) {
            self.finalize(id, termination_for(&reason));
            return StepDirective::Stop { reason };
        }
        if let Some(reason) = self.yield_reason_now(id) {
            self.do_yield(id, &reason);
            return StepDirective::Yield { reason };
        }
        let steering = self
            .inner
            .st
            .tasks
            .get_mut(id)
            .map(|r| std::mem::take(&mut r.steering))
            .unwrap_or_default();
        for env in &steering {
            let latency = steps.saturating_sub(env.sent_at_step);
            self.event(
                EVENT_STEERED,
                id,
                json!({ "seq": env.seq, "kind": env.steer.kind(), "latency_steps": latency, "at_step": steps }),
            );
        }
        StepDirective::Continue { steering }
    }

    /// Czy zadanie trzeba zatrzymać teraz (prośba, odebrane zasoby, budżet, termin).
    fn stop_reason_now(&self, id: &TaskId) -> Option<StopReason> {
        let rec = self.inner.st.tasks.get(id)?;
        if let Some(reason) = &rec.stop_request {
            return Some(reason.clone());
        }
        let revoked = self
            .inner
            .leases
            .get(id)
            .is_some_and(|ls| ls.iter().any(scheduler_lite_contract::Lease::is_revoked));
        if revoked {
            return Some(StopReason::Cancelled {
                cause: CancelCause::KillSwitch,
            });
        }
        let b = &rec.spec.budget;
        let running_ms = match rec.state {
            TaskState::Running { since_ms, .. } => self.now.saturating_sub(since_ms),
            _ => 0,
        };
        let budget = if rec.steps >= b.max_steps {
            Some(BudgetKind::Steps)
        } else if rec.wall_ms.saturating_add(running_ms) >= b.max_wall_ms {
            Some(BudgetKind::Wall)
        } else if b.max_cost_micro_pln.is_some_and(|m| rec.cost_micro_pln > m) {
            Some(BudgetKind::Cost)
        } else {
            None
        };
        if let Some(budget) = budget {
            return Some(StopReason::Budget { budget });
        }
        (self.now >= rec.deadline_ms).then_some(StopReason::Deadline)
    }

    /// Czy zadanie ma oddać zasoby (pauza, wywłaszczenie przez mowę/wyższą klasę, warunek okna).
    fn yield_reason_now(&self, id: &TaskId) -> Option<YieldReason> {
        let rec = self.inner.st.tasks.get(id)?;
        if let Some(reason) = &rec.yield_request {
            return Some(reason.clone());
        }
        let preempted = self.inner.leases.get(id).and_then(|ls| {
            ls.iter()
                .find(|l| l.preempt_requested())
                .map(|l| l.resource().clone())
        });
        if let Some(resource) = preempted {
            return Some(YieldReason::Preempted {
                resource: Some(resource),
            });
        }
        let c = self.inner.st.conditions;
        let w = &rec.spec.window;
        ((w.only_when_idle && !c.user_idle) || (w.not_in_game_mode && c.game_mode))
            .then_some(YieldReason::ConditionLost)
    }

    /// Wynik wykonawczyni. Spóźniony (stare wysłanie) → ignorowany.
    pub(crate) fn finish(&mut self, dispatch: DispatchId, result: WorkerResult) {
        let Some(id) = self.running_task(dispatch) else {
            return;
        };
        match result {
            WorkerResult::Succeeded { output } => {
                self.add_step(&id);
                if output_size(&output) > MAX_OUTPUT_BYTES {
                    let attempts = self.attempt_of(&id);
                    self.finalize(
                        &id,
                        Termination::Failed {
                            error: "wynik zadania jest za duży".into(),
                            attempts,
                        },
                    );
                } else {
                    self.finalize(&id, Termination::Succeeded { output });
                }
            }
            WorkerResult::Failed { error, retryable } => {
                self.add_step(&id);
                self.fail(&id, error, retryable);
            }
            WorkerResult::Yielded => self.do_yield(&id, &YieldReason::Preempted { resource: None }),
            WorkerResult::Stopped => self.finalize(
                &id,
                Termination::Cancelled {
                    cause: CancelCause::Worker,
                },
            ),
        }
    }

    fn add_step(&mut self, id: &TaskId) {
        if let Some(rec) = self.inner.st.tasks.get_mut(id) {
            rec.steps = rec.steps.saturating_add(1);
        }
    }

    fn attempt_of(&self, id: &TaskId) -> u32 {
        self.inner
            .st
            .tasks
            .get(id)
            .map_or(1, |r| r.failures.saturating_add(1))
    }

    /// Porażka: ponowienie z odstępem (jeśli wolno i zdąży przed terminem) albo koniec.
    fn fail(&mut self, id: &TaskId, error: String, retryable: bool) {
        let Some(rec) = self.inner.st.tasks.get_mut(id) else {
            return;
        };
        rec.failures = rec.failures.saturating_add(1);
        let failures = rec.failures;
        let backoff = rec.spec.retry.backoff_ms(failures);
        let until = self.now.saturating_add(backoff);
        let retry = retryable && failures < rec.spec.retry.max_attempts && until < rec.deadline_ms;
        if !retry {
            self.finalize(
                id,
                Termination::Failed {
                    error,
                    attempts: failures,
                },
            );
            return;
        }
        self.close_running(id);
        drop(self.inner.leases.remove(id));
        if let Some(rec) = self.inner.st.tasks.get_mut(id) {
            rec.state = TaskState::RetryWait {
                until_ms: until,
                last_error: error.chars().take(200).collect(),
            };
            rec.blocked = Some(BlockReason::Backoff { until_ms: until });
        }
        self.event(
            EVENT_RETRY,
            id,
            json!({ "attempt": failures + 1, "at_ms": until, "backoff_ms": backoff }),
        );
        self.out.dirty = true;
    }
}
