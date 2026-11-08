//! Wyzwalanie: terminy czasowe (z zaległościami), wejścia zdarzeniowe, ręczne; okno ciszy i DND,
//! limity częstości, ochrona przed pętlą łańcucha, budowa zadania (pochodzenie, taint, sufit
//! uprawnień, treść niezaufana osobno) i dziennik uruchomień.

use safety_broker_contract::TaintSource;
use scheduler_contract::{RetryPolicy, TaskId, TaskOrigin, TaskSpec, TimeWindow, validate_spec};
use serde_json::{Value, json};

use crate::engine::input::{Match, matches};
use crate::engine::{Deferred, TriggerEngine, next_time};
use crate::record::{FireCause, RunOutcome, RunRecord, SuppressReason, TriggerInput};
use crate::spec::{Actor, QuietHours, QuietMode, TriggerId, TriggerSpec};
use crate::tz::Tz;
use crate::validate::{
    DEFAULT_TASK_DEADLINE_MS, GLOBAL_MAX_FIRES_PER_HOUR, LOG_PER_TRIGGER, LOG_TOTAL, TriggerError,
    may_manage,
};

/// Spóźnienie, od którego wystąpienie czasowe uznaje się za przegapione (1 min).
const MISFIRE_GRACE_MS: u64 = 60_000;
/// Co ile sprawdzać ponownie odłożone przez DND.
const DND_RECHECK_MS: u64 = 60_000;

/// Ujście zadań (scheduler).
pub trait TaskSink {
    /// Zgłasza zadanie; błąd = odrzucenie przez scheduler (zapis w dzienniku).
    fn submit(&self, task: TaskSpec) -> Result<TaskId, String>;
}

fn base_cause(cause: &FireCause) -> &FireCause {
    match cause {
        FireCause::Deferred { original } => base_cause(original),
        other => other,
    }
}

/// Taint i treść niezaufana przyczyny (osobno od celu — runtime ją delimituje).
fn untrusted(cause: &FireCause) -> (Vec<TaintSource>, Value) {
    match base_cause(cause) {
        FireCause::File { path } => (
            vec![TaintSource::File],
            json!({ "source": "file", "content": path }),
        ),
        FireCause::Message { session, turn } => (
            vec![TaintSource::Email],
            json!({ "source": "message", "session": session, "turn": turn }),
        ),
        FireCause::TaskFinished {
            task,
            result,
            taint,
            ..
        } if !taint.is_empty() => (
            taint.clone(),
            json!({ "source": "task", "task": task, "result": result }),
        ),
        _ => (Vec::new(), Value::Null),
    }
}

/// Zadanie dla wyzwolenia: pochodzenie zawsze `Trigger` (także harmonogram — most odmawia; CX-d),
/// klasa najwyżej `Agent`, taint z przyczyny, `scope` jako sufit.
pub fn task_for(spec: &TriggerSpec, seq: u64, cause: &FireCause, now: u64) -> TaskSpec {
    let a = &spec.action;
    let depth = match base_cause(cause) {
        FireCause::TaskFinished { depth, .. } => depth.saturating_add(1),
        _ => 1,
    };
    let origin = TaskOrigin::Trigger {
        trigger_id: spec.id.to_string(),
        depth,
    };
    let (taint, content) = untrusted(cause);
    let session = match base_cause(cause) {
        FireCause::Message { session, .. } => Some(session.clone()),
        _ => None,
    };
    let mut task = TaskSpec::new(
        format!("trig/{}/{seq}", spec.id),
        a.title.clone(),
        a.assignee.clone(),
        a.class.min(scheduler_contract::TaskClass::Agent),
        origin,
    );
    task.executor = a.executor.clone();
    task.resources = a.resources.clone();
    task.budget = a.budget;
    task.retry = RetryPolicy::default();
    task.window = TimeWindow {
        deadline_ms: Some(
            now.saturating_add(a.deadline_after_ms.unwrap_or(DEFAULT_TASK_DEADLINE_MS)),
        ),
        only_when_idle: a.only_when_idle,
        ..TimeWindow::default()
    };
    task.taint = taint;
    task.session = session;
    task.payload = json!({
        "goal": a.goal,
        "trigger": { "id": spec.id, "name": spec.name, "kind": spec.kind.name(), "fired_at_ms": now },
        "scope_ceiling": spec.scope,
        "untrusted": content,
    });
    task
}

/// Sprawdza, czy zadanie wyzwalacza przejdzie walidację schedulera (już przy tworzeniu).
pub(crate) fn probe(spec: &TriggerSpec, now: u64) -> Result<(), TriggerError> {
    let cause = FireCause::Manual {
        by: spec.owner.clone(),
    };
    validate_spec(&task_for(spec, 1, &cause, now), now).map_err(|e| TriggerError::Invalid {
        id: spec.id.clone(),
        reason: e.to_string(),
    })
}

/// Koniec okna ciszy, jeśli `now` w nim leży.
fn quiet_until(q: &QuietHours, tz: &Tz, now: u64) -> Option<u64> {
    let (weekday, minute) = tz.weekday_minute(i64::try_from(now).ok()?)?;
    let day = u8::try_from(weekday.num_days_from_sunday()).ok()?;
    if !q.days.is_empty() && !q.days.contains(&day) {
        return None;
    }
    let inside = if q.start_min <= q.end_min {
        (q.start_min..q.end_min).contains(&minute)
    } else {
        minute >= q.start_min || minute < q.end_min
    };
    if !inside {
        return None;
    }
    let left = (u64::from(q.end_min) + 1440 - u64::from(minute)) % 1440;
    Some(now - now % 60_000 + left.max(1) * 60_000)
}

impl TriggerEngine {
    fn record(&mut self, id: &TriggerId, record: RunRecord) -> RunRecord {
        if let Some(rec) = self.st.triggers.get_mut(id) {
            if matches!(record.outcome, RunOutcome::Suppressed { .. }) {
                rec.suppressed += 1;
            }
            rec.log.push_back(record.clone());
            while rec.log.len() > LOG_PER_TRIGGER {
                rec.log.pop_front();
            }
        }
        self.st.log.push_back(record.clone());
        while self.st.log.len() > LOG_TOTAL {
            self.st.log.pop_front();
        }
        record
    }

    /// Próba wyzwolenia: cisza/DND (chyba że ręcznie), limity, zgłoszenie zadania.
    /// `None` = ponowne odłożenie bez wpisu (już było odłożone).
    fn attempt(
        &mut self,
        id: &TriggerId,
        cause: FireCause,
        now: u64,
        sink: &dyn TaskSink,
        manual: bool,
    ) -> Option<RunRecord> {
        let rec = self.st.triggers.get(id)?;
        let was_deferred = matches!(cause, FireCause::Deferred { .. });
        if !manual {
            let quiet = rec
                .spec
                .quiet
                .as_ref()
                .and_then(|q| quiet_until(q, &rec.spec.tz, now));
            let dnd = (self.st.dnd && rec.spec.respect_dnd).then(|| now + DND_RECHECK_MS);
            if let Some(until) = quiet.or(dnd) {
                let reason = if quiet.is_some() {
                    SuppressReason::Quiet
                } else {
                    SuppressReason::Dnd
                };
                return self.quiet(id, cause, until, reason, now, was_deferred);
            }
        }
        let rec = self.st.triggers.get_mut(id)?;
        rec.recent.retain(|t| t + rec.spec.rate.per_ms > now);
        self.st.global_recent.retain(|t| t + 3_600_000 > now);
        let limited = if rec.recent.len() >= rec.spec.rate.max_fires as usize {
            Some(SuppressReason::RateLimited)
        } else if self.st.global_recent.len() >= GLOBAL_MAX_FIRES_PER_HOUR {
            Some(SuppressReason::GlobalRateLimited)
        } else {
            None
        };
        if let Some(reason) = limited {
            return Some(self.suppress(id, cause, reason, now));
        }
        rec.seq += 1;
        let task = task_for(&rec.spec, rec.seq, &cause, now);
        let outcome = match sink.submit(task) {
            Ok(task) => {
                rec.recent.push(now);
                rec.fired += 1;
                rec.last_fire_ms = Some(now);
                self.st.global_recent.push(now);
                RunOutcome::Submitted { task }
            }
            Err(error) => RunOutcome::Failed { error },
        };
        Some(self.record(
            id,
            RunRecord {
                at_ms: now,
                trigger: id.clone(),
                cause,
                outcome,
            },
        ))
    }

    fn suppress(
        &mut self,
        id: &TriggerId,
        cause: FireCause,
        reason: SuppressReason,
        now: u64,
    ) -> RunRecord {
        self.record(
            id,
            RunRecord {
                at_ms: now,
                trigger: id.clone(),
                cause,
                outcome: RunOutcome::Suppressed { reason },
            },
        )
    }

    fn quiet(
        &mut self,
        id: &TriggerId,
        cause: FireCause,
        until: u64,
        reason: SuppressReason,
        now: u64,
        was_deferred: bool,
    ) -> Option<RunRecord> {
        let rec = self.st.triggers.get_mut(id)?;
        if rec.spec.quiet_mode == QuietMode::Skip {
            return Some(self.suppress(id, cause, reason, now));
        }
        let original = match cause {
            FireCause::Deferred { original } => original,
            other => Box::new(other),
        };
        let fresh = rec.deferred.is_none() && !was_deferred;
        rec.deferred = Some(Deferred {
            until_ms: until,
            cause: FireCause::Deferred {
                original: original.clone(),
            },
        });
        fresh.then(|| {
            self.record(
                id,
                RunRecord {
                    at_ms: now,
                    trigger: id.clone(),
                    cause: *original,
                    outcome: RunOutcome::Deferred { until_ms: until },
                },
            )
        })
    }

    /// Upływ czasu: odłożone i terminy czasowe (zaległe wg `misfire`).
    pub fn tick(&mut self, now: u64, sink: &dyn TaskSink) -> Vec<RunRecord> {
        let ids: Vec<TriggerId> = self.st.triggers.keys().cloned().collect();
        let mut out = Vec::new();
        for id in ids {
            let Some(rec) = self.st.triggers.get_mut(&id) else {
                continue;
            };
            if !rec.spec.enabled {
                continue;
            }
            if let Some(d) = rec.deferred.take_if(|d| d.until_ms <= now) {
                out.extend(self.attempt(&id, d.cause, now, sink, false));
            }
            out.extend(self.due(&id, now, sink));
        }
        out
    }

    fn due(&mut self, id: &TriggerId, now: u64, sink: &dyn TaskSink) -> Option<RunRecord> {
        let rec = self.st.triggers.get_mut(id)?;
        let scheduled = rec.next_fire_ms.filter(|t| *t <= now)?;
        let mut missed = 0u32;
        let mut t = scheduled;
        while let Some(n) = next_time(&rec.spec, rec.created_at_ms, t).filter(|n| *n <= now) {
            missed += 1;
            t = n;
            if missed >= 10_000 {
                break;
            }
        }
        rec.next_fire_ms = next_time(&rec.spec, rec.created_at_ms, now);
        let late = now.saturating_sub(scheduled) > MISFIRE_GRACE_MS;
        let cause = FireCause::Time {
            scheduled_ms: scheduled,
            missed: if late { missed + 1 } else { missed },
        };
        if late && rec.spec.misfire == crate::spec::MisfirePolicy::Skip {
            return Some(self.suppress(id, cause, SuppressReason::Missed, now));
        }
        self.attempt(id, cause, now, sink, false)
    }

    /// Wejście zdarzeniowe → wyzwolenia pasujących, włączonych wyzwalaczy.
    pub fn on_input(
        &mut self,
        input: &TriggerInput,
        now: u64,
        sink: &dyn TaskSink,
    ) -> Vec<RunRecord> {
        let ids: Vec<TriggerId> = self.st.triggers.keys().cloned().collect();
        let mut out = Vec::new();
        for id in ids {
            let Some(rec) = self.st.triggers.get(&id).filter(|r| r.spec.enabled) else {
                continue;
            };
            match matches(rec, input) {
                Match::No => {}
                Match::Suppress(cause, reason) => out.push(self.suppress(&id, cause, reason, now)),
                Match::Fire(cause) => out.extend(self.attempt(&id, cause, now, sink, false)),
            }
        }
        out
    }

    /// „Uruchom teraz” (właścicielka albo użytkownik); pomija ciszę, nie pomija limitów.
    pub fn fire_manual(
        &mut self,
        id: &TriggerId,
        actor: &Actor,
        now: u64,
        sink: &dyn TaskSink,
    ) -> Result<RunRecord, TriggerError> {
        let rec = self
            .st
            .triggers
            .get(id)
            .ok_or_else(|| TriggerError::Unknown(id.clone()))?;
        if !may_manage(actor, &rec.spec.owner) {
            return Err(TriggerError::Forbidden(
                "uruchomienie cudzego wyzwalacza".into(),
            ));
        }
        if !rec.spec.enabled {
            return Err(TriggerError::Disabled(id.clone()));
        }
        let cause = FireCause::Manual { by: actor.clone() };
        self.attempt(id, cause, now, sink, true)
            .ok_or_else(|| TriggerError::Unknown(id.clone()))
    }
}
