//! Odnalezienie zapisanego wykonania po jawnej zgodzie „Kontynuuj”.
use crate::{ChatEngine, GenRequest};
use agent_runtime_contract::{Checkpoint, RunBudget, RunOutcome};
use app_api::ids;

pub(crate) fn saved_native(core: &ChatEngine, req: &GenRequest) -> Option<Checkpoint> {
    let old = ids::turn_dto(&req.session, req.continues?);
    core.inner
        .store
        .work_for_turn(&req.session, &old)
        .ok()?
        .into_iter()
        .filter(|(_, kind, _)| kind == "runtime")
        .filter_map(|(_, _, v)| serde_json::from_value::<Checkpoint>(v).ok())
        .find(|cp| {
            cp.options.parent.is_none()
                && (cp.finished.is_none()
                    || matches!(cp.finished, Some(RunOutcome::BudgetExceeded { .. })))
        })
}
pub(crate) fn next_budget(mut base: RunBudget, cp: &Checkpoint) -> RunBudget {
    base.max_steps = base
        .max_steps
        .saturating_add(cp.usage.steps)
        .saturating_add(cp.delegated.steps);
    base.max_tokens = base
        .max_tokens
        .saturating_add(cp.usage.tokens())
        .saturating_add(cp.delegated.tokens());
    base.max_wall_ms = base.max_wall_ms.saturating_add(cp.usage.elapsed_ms);
    base
}

impl ChatEngine {
    /// Saved task identity, including a crash before its assistant turn was committed.
    pub fn recovery_agent(
        &self,
        session: &sessions_contract::SessionId,
        turn: sessions_contract::TurnId,
    ) -> Option<String> {
        let owner = ids::turn_dto(session, turn);
        let rows = self.inner.store.work_for_turn(session, &owner).ok()?;
        for (_, kind, v) in &rows {
            if kind == "conductor"
                && let Ok(s) = serde_json::from_value::<crate::conductor_state::Saved>(v.clone())
                && !s.complete
            {
                return Some(s.parent.agent);
            }
        }
        rows.into_iter()
            .filter(|(_, k, _)| k == "runtime")
            .filter_map(|(_, _, v)| serde_json::from_value::<Checkpoint>(v).ok())
            .find(|c| {
                c.options.parent.is_none()
                    && (c.finished.is_none()
                        || matches!(c.finished, Some(RunOutcome::BudgetExceeded { .. })))
            })
            .map(|c| c.spec.agent.to_string())
    }
    /// Only persisted assistant turns can be continuation links (never a reserved self id).
    pub(crate) fn continuation_link(
        &self,
        req: &GenRequest,
        new: sessions_contract::TurnId,
    ) -> Option<sessions_contract::TurnId> {
        req.continues.filter(|old| {
            *old != new
                && self
                    .inner
                    .store
                    .meta(&req.session, *old)
                    .ok()
                    .flatten()
                    .is_some_and(|m| m.agent.is_some())
        })
    }
    /// Stale Replay entries become resumable after restart; active work is never relabelled.
    pub fn recovery_runs(
        &self,
        session: &sessions_contract::SessionId,
    ) -> Result<Vec<app_api::dto::AgentRunDetail>, app_api::AppError> {
        let mut runs = self.inner.store.runs(session)?;
        if self.generation(session).is_some() {
            return Ok(runs);
        }
        for detail in &mut runs {
            let r = &mut detail.run;
            if r.parent_id.is_some() || r.task_id.is_some() {
                continue;
            }
            if let Some(raw) = r.turn_id.as_deref()
                && let Ok(turn) = ids::parse_turn_in(session, raw)
                && self.recovery_agent(session, turn).is_some()
            {
                r.state = app_api::dto::RunState::Paused;
                r.summary =
                    Some("Postęp zapisany lokalnie. Możesz wznowić od tego miejsca.".into());
            }
        }
        Ok(runs)
    }
}

impl ChatEngine {
    pub(crate) fn validate_recovery(
        &self,
        session: &sessions_contract::SessionId,
        turn: sessions_contract::TurnId,
    ) -> Result<bool, app_api::AppError> {
        let rows = self
            .inner
            .store
            .work_for_turn(session, &ids::turn_dto(session, turn))?;
        let mut exists = false;
        for (_, kind, value) in rows {
            match kind.as_str() {
                "runtime" => {
                    let _: Checkpoint =
                        serde_json::from_value(value).map_err(app_api::AppError::storage)?;
                    exists = true;
                }
                "conductor" => {
                    let _: crate::conductor_state::Saved =
                        serde_json::from_value(value).map_err(app_api::AppError::storage)?;
                    exists = true;
                }
                _ => {}
            }
        }
        let paused = self
            .inner
            .store
            .meta(session, turn)?
            .and_then(|m| m.error)
            .is_some_and(|e| e.code == app_api::dto::TurnErrorCode::TaskPaused);
        if paused && !exists {
            return Err(app_api::AppError::storage(
                "Brak czytelnego lokalnego zapisu tego wstrzymanego zadania. Nie rozpoczynam go ponownie.",
            ));
        }
        Ok(exists)
    }
    /// Follow the durable continuation chain; an old button cannot replay completed work.
    pub fn resolve_continuation(
        &self,
        session: &sessions_contract::SessionId,
        mut turn: sessions_contract::TurnId,
    ) -> Result<sessions_contract::TurnId, app_api::AppError> {
        use sessions_contract::SessionHistory;
        for _ in 0..64 {
            let key = format!("continued-{}", ids::turn_dto(session, turn));
            let Some(value) = self.inner.store.work(session, &key)? else {
                return Ok(turn);
            };
            let next = value["turn"]
                .as_u64()
                .map(sessions_contract::TurnId)
                .ok_or_else(|| app_api::AppError::storage("Nieprawidłowy zapis kontynuacji"))?;
            if self.recovery_agent(session, next).is_some() {
                turn = next;
                continue;
            }
            if self.inner.sessions.turn(session, next).is_ok() {
                return Err(app_api::AppError::invalid(
                    "Ten zapis został już wznowiony. Użyj najnowszego wyniku zadania.",
                ));
            }
            return Ok(turn);
        }
        Err(app_api::AppError::invalid(
            "Zbyt długi łańcuch kontynuacji.",
        ))
    }
    pub(crate) fn claim_continuation(
        &self,
        req: &GenRequest,
        new: sessions_contract::TurnId,
    ) -> Result<(), app_api::AppError> {
        if let Some(old) = req.continues.filter(|old| *old != new) {
            self.inner.store.save_work(
                &req.session,
                &format!("continued-{}", ids::turn_dto(&req.session, old)),
                "continuation",
                None,
                &serde_json::json!({"turn":new.0}),
            )?;
        }
        Ok(())
    }
}

impl ChatEngine {
    /// Continue a saved task after the host acquired the session lock.
    pub async fn continue_saved(
        &self,
        id: sessions_contract::SessionId,
        target: sessions_contract::TurnId,
    ) -> Result<String, app_api::AppError> {
        use crate::Placement;
        use app_api::AppError;
        use sessions_contract::SessionHistory;
        if let Some(running) = self.generation(&id) {
            // Zdarzenie końca tury wychodzi chwilę przed zwolnieniem sesji: prośba o kontynuację
            // wysłana zaraz po nim nie może dostać fałszywego „już pracuje” (verify 0.0.75).
            running.wait(std::time::Duration::from_millis(500)).await;
            if self.generation(&id).is_some() {
                return Err(AppError::invalid(
                    "Zadanie już pracuje. Poczekaj na jego wynik.",
                ));
            }
        }
        let target = self.resolve_continuation(&id, target)?;
        let expected = self.validate_recovery(&id, target)?;
        let saved = self.recovery_agent(&id, target);
        if expected && saved.is_none() {
            return Err(AppError::invalid(
                "Zapis nie pozwala na wznowienie tego zadania. Nie rozpoczynam go ponownie.",
            ));
        }
        let turn = self.inner.sessions.turn(&id, target);
        let (leaf, agent) = if let Some(agent) = saved {
            let leaf = if turn
                .as_ref()
                .is_ok_and(|t| matches!(t.author, sessions_contract::Author::Agent { .. }))
            {
                target
            } else {
                self.inner
                    .sessions
                    .active_leaf(&id)?
                    .ok_or_else(|| AppError::invalid("Brak historii zapisanego zadania."))?
            };
            (leaf, agent)
        } else {
            let turn = turn?;
            (
                target,
                crate::project::author_of(&turn, self.inner.store.meta(&id, turn.id)?.as_ref()),
            )
        };
        self.start_generation(GenRequest {
            session: id,
            placement: Placement::Child(leaf),
            history_leaf: leaf,
            agent,
            profile: None,
            continues: Some(target),
            origin: risk_classifier_contract::CommandOrigin::UserText,
            tap: None,
        })
        .await
    }
}
