//! Dopasowanie wejść zdarzeniowych do wyzwalaczy (plik, wiadomość, koniec zadania) z ochroną
//! przed pętlą łańcucha wyzwalaczy.

use scheduler_contract::TaskOrigin;

use crate::engine::TrigRec;
use crate::record::{FireCause, SuppressReason, TriggerInput, file_matches};
use crate::spec::{FinishFilter, TriggerKind};
use crate::validate::MAX_CHAIN_DEPTH;

pub(crate) enum Match {
    No,
    Fire(FireCause),
    Suppress(FireCause, SuppressReason),
}

pub(crate) fn matches(rec: &TrigRec, input: &TriggerInput) -> Match {
    match (&rec.spec.kind, input) {
        (TriggerKind::FileInDir { dir, pattern }, TriggerInput::FileCreated { path })
            if file_matches(dir, pattern.as_deref(), path) =>
        {
            Match::Fire(FireCause::File { path: path.clone() })
        }
        (
            TriggerKind::NewMessage { session: filter },
            TriggerInput::NewMessage {
                session,
                turn,
                role,
            },
        ) if role != "assistant" && filter.as_ref().is_none_or(|s| s == session) => {
            Match::Fire(FireCause::Message {
                session: session.clone(),
                turn: turn.clone(),
            })
        }
        (
            TriggerKind::TaskFinished {
                task_prefix,
                outcome,
            },
            TriggerInput::TaskFinished {
                task,
                result,
                origin,
                taint,
            },
        ) => {
            let prefix_ok = task_prefix
                .as_ref()
                .is_none_or(|p| task.as_str().starts_with(p.as_str()));
            let failed = matches!(
                result.as_str(),
                "failed" | "expired" | "budget_exceeded" | "budget_blocked"
            );
            let outcome_ok = match outcome {
                FinishFilter::Any => true,
                FinishFilter::Succeeded => result == "succeeded",
                FinishFilter::Failed => failed,
            };
            if !(prefix_ok && outcome_ok) {
                return Match::No;
            }
            let depth = origin.trigger_depth();
            let cause = FireCause::TaskFinished {
                task: task.clone(),
                result: result.clone(),
                depth,
                taint: taint.clone(),
            };
            let own = matches!(origin, TaskOrigin::Trigger { trigger_id, .. } if *trigger_id == rec.spec.id.0)
                || matches!(origin, TaskOrigin::Schedule { schedule_id } if *schedule_id == rec.spec.id.0);
            if own {
                Match::Suppress(cause, SuppressReason::SelfLoop)
            } else if depth >= MAX_CHAIN_DEPTH {
                Match::Suppress(cause, SuppressReason::ChainTooDeep)
            } else {
                Match::Fire(cause)
            }
        }
        _ => Match::No,
    }
}
