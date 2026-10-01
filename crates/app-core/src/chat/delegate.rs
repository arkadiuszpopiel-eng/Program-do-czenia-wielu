//! Delegacja z czatu do mostu CLI („Delta, zleć to Claude Code", „przekaż Codexowi: …"):
//! tylko jawne polecenie w wiadomości użytkownika (tekst albo głos) — nigdy treść agentki,
//! wyzwalacza ani niezaufana. Zadanie mostu (pochodzenie `User`) idzie przez scheduler
//! (panel Zadania); Replay mostu oznaczony „niezweryfikowane przez Alfę", prośby o uprawnienia
//! → Broker; tura agentki = wynik mostu z dopiskiem. Anulowanie tury anuluje zadanie mostu.

use std::time::Instant;

use agent_backends_contract::BridgeKind;
use risk_classifier_contract::CommandOrigin;
use sessions_contract::{Role, SessionHistory};

use super::GenRequest;
use super::project::render_closed;
use super::stream::Outcome;
use crate::core::{AppCore, GenHandle};
use crate::dto::{AlfaEvent, StopReason, TurnError, TurnErrorCode, TurnStatus};
use crate::ids;

/// Rozpoznana delegacja.
pub(crate) struct Delegation {
    kind: BridgeKind,
    goal: String,
}

impl AppCore {
    /// Delegacja w ostatniej wiadomości użytkownika („to" = poprzednia wiadomość użytkownika).
    pub(crate) fn delegation(&self, req: &GenRequest) -> Option<Delegation> {
        let from_user = matches!(
            req.origin,
            CommandOrigin::UserText | CommandOrigin::UserVoice { .. }
        );
        if req.continues.is_some() || !from_user {
            return None;
        }
        let path = self
            .inner
            .sessions
            .branch_projection(&req.session, req.history_leaf)
            .ok()?;
        let mut users = path
            .iter()
            .rev()
            .filter(|t| t.role == Role::User)
            .map(|t| t.content.text.trim().to_owned());
        let (kind, prompt) = app_bridges::parse_delegation(&users.next()?)?;
        let goal = match prompt {
            Some(p) => p,
            None => users.find(|t| !t.is_empty()).unwrap_or_default(),
        };
        Some(Delegation { kind, goal })
    }
}

fn outcome(text: String, status: TurnStatus, stop: StopReason, error: Option<String>) -> Outcome {
    let mut out = Outcome::failed(TurnError {
        code: TurnErrorCode::Provider,
        message: error.clone().unwrap_or_default(),
        retry_at: None,
        provider: None,
    });
    out.text = text;
    out.status = status;
    if error.is_none() {
        out.error = None;
        out.stop = Some(stop);
    }
    out
}

/// Zadanie mostu jako odpowiedź na turę.
pub(crate) async fn run(
    core: &AppCore,
    req: &GenRequest,
    handle: &GenHandle,
    d: Delegation,
) -> Outcome {
    let started = Instant::now();
    let cancel = handle.cancel.clone();
    let done = core
        .inner
        .tasks
        .delegate(&req.session, &req.agent, d.kind, &d.goal, async move {
            cancel.cancelled().await;
        })
        .await;
    let text = match done.result {
        Ok(text) => text,
        Err(_) if handle.cancel.is_cancelled() => {
            return outcome(
                String::new(),
                TurnStatus::Cancelled,
                StopReason::Cancelled,
                None,
            );
        }
        Err(message) => {
            let message = format!("Most CLI (zadanie {}): {message}", done.task);
            return outcome(
                String::new(),
                TurnStatus::Error,
                StopReason::End,
                Some(message),
            );
        }
    };
    let blocks = render_closed(&text);
    if let Ok(mut live) = handle.live.lock() {
        live.text.clone_from(&text);
        live.blocks.clone_from(&blocks);
    }
    if let Some(tap) = &req.tap {
        let _ = tap.send(crate::ports::VoiceChunk::Text(text.clone()));
    }
    core.emit(AlfaEvent::TextDelta {
        session_id: req.session.to_string(),
        turn_id: ids::turn_dto(&req.session, handle.turn),
        text: text.clone(),
        blocks,
    });
    let mut out = outcome(text, TurnStatus::Complete, StopReason::End, None);
    out.latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    out
}
