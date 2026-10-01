//! Upływ czasu dla zadań: terminy, budżet czasu, wymuszone przerwanie po czasie na punkt
//! atomowy, wygaśnięcie czekających, koniec odstępu ponowienia.

use serde_json::json;

use crate::engine::run::termination_for;
use crate::engine::{Ctx, SchedEffect};
use crate::events::EVENT_ABORTED;
use crate::host::SchedHost;
use crate::ids::{DispatchId, TaskId};
use crate::state::{BlockReason, BudgetKind, ExpiryReason, TaskState, Termination};
use crate::steer::StopReason;
use crate::validate::STOP_GRACE_MS;

impl<H: SchedHost> Ctx<'_, H> {
    /// Terminy, budżet czasu i wymuszone przerwanie zadań w toku.
    pub(crate) fn enforce_running(&mut self) -> bool {
        let running: Vec<(TaskId, DispatchId)> = self
            .inner
            .st
            .tasks
            .iter()
            .filter_map(|(id, r)| r.running_dispatch().map(|d| (id.clone(), d)))
            .collect();
        let mut progressed = false;
        for (id, dispatch) in running {
            let Some(rec) = self.inner.st.tasks.get(&id) else {
                continue;
            };
            if rec.stop_request.is_none() {
                let running_ms = match rec.state {
                    TaskState::Running { since_ms, .. } => self.now.saturating_sub(since_ms),
                    _ => 0,
                };
                if self.now >= rec.deadline_ms {
                    self.request_stop(&id, StopReason::Deadline);
                } else if rec.wall_ms.saturating_add(running_ms) >= rec.spec.budget.max_wall_ms {
                    self.request_stop(
                        &id,
                        StopReason::Budget {
                            budget: BudgetKind::Wall,
                        },
                    );
                }
            }
            let Some(rec) = self.inner.st.tasks.get(&id) else {
                continue;
            };
            let overdue = rec
                .stop_requested_at_ms
                .is_some_and(|at| self.now >= at.saturating_add(STOP_GRACE_MS));
            if let (true, Some(reason)) = (overdue, rec.stop_request.clone()) {
                self.abort(&id, dispatch, &reason);
                progressed = true;
            }
        }
        progressed
    }

    /// Przerwanie siłą: efekt `Abort` przed zwolnieniem zasobów, potem zakończenie.
    pub(crate) fn abort(&mut self, id: &TaskId, dispatch: DispatchId, reason: &StopReason) {
        self.out.effects.push(SchedEffect::Abort {
            task: id.clone(),
            dispatch,
        });
        self.event(EVENT_ABORTED, id, json!({ "reason": reason }));
        self.finalize(id, termination_for(reason));
    }

    /// Termin minął zanim zadanie ruszyło (czeka, wstrzymane, odstęp ponowienia).
    pub(crate) fn expire_waiting(&mut self) -> bool {
        let expired: Vec<(TaskId, Option<BlockReason>)> = self
            .inner
            .st
            .tasks
            .iter()
            .filter(|(_, r)| {
                !r.state.is_terminal()
                    && r.running_dispatch().is_none()
                    && self.now >= r.deadline_ms
            })
            .map(|(id, r)| (id.clone(), r.blocked.clone()))
            .collect();
        let progressed = !expired.is_empty();
        for (id, blocked) in expired {
            self.finalize(
                &id,
                Termination::Expired {
                    reason: ExpiryReason::NotStarted { blocked },
                },
            );
        }
        progressed
    }

    /// Koniec odstępu ponowienia → gotowe.
    pub(crate) fn wake_retries(&mut self) -> bool {
        let mut progressed = false;
        for rec in self.inner.st.tasks.values_mut() {
            if let TaskState::RetryWait { until_ms, .. } = rec.state
                && until_ms <= self.now
            {
                rec.state = TaskState::Ready;
                rec.blocked = None;
                progressed = true;
            }
        }
        if progressed {
            self.out.dirty = true;
        }
        progressed
    }
}
