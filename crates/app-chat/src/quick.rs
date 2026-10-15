//! Polecenia bez modelu (P5, `lib-quick`): rachunki, literowanie, losowanie, data i godzina.
//! Tylko jawna wiadomość użytkownika (tekst albo głos) — nigdy treść agentki, wyzwalacza ani
//! kontynuacja. Odpowiedź liczona lokalnie, natychmiast i także offline (wtedy wiadomość nie
//! trafia do kolejki); bez dostawcy, kosztu i tokenów. Wyłączane ustawieniem
//! `sessions.local_commands`.

use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use lib_quick::Reply;
use risk_classifier_contract::CommandOrigin;
use sessions_contract::{Role, SessionHistory};

use app_api::dto::{AlfaEvent, StopReason, TurnStatus};
use app_api::ids;

use crate::engine::{ChatEngine, GenHandle};
use crate::generate::GenRequest;
use crate::outcome::Outcome;
use crate::project::render_closed;

/// Ustawienie: polecenia bez modelu włączone (domyślnie tak).
pub const LOCAL_COMMANDS_KEY: &str = "sessions.local_commands";

/// Liczba losowa `0..n` (kostka, moneta — nie do kryptografii): losowo kluczowany SipHash.
fn roll(n: u64) -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    hasher.write_u128(nanos);
    hasher.finish() % n.max(1)
}

fn now() -> chrono::NaiveDateTime {
    chrono::Local::now().naive_local()
}

impl ChatEngine {
    /// Czy wiadomość użytkownika dostanie odpowiedź bez modelu (np. offline — bez kolejki).
    pub async fn answers_locally(&self, text: &str) -> bool {
        lib_quick::matches(text) && self.config_bool(LOCAL_COMMANDS_KEY, true).await
    }

    /// Polecenie w ostatniej wiadomości użytkownika (tylko świeża tura z tekstu albo głosu).
    pub(crate) async fn quick(&self, req: &GenRequest) -> Option<Reply> {
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
        let last = path.last().filter(|t| t.role == Role::User)?;
        let text = last.content.text.trim().to_owned();
        if !last.content.blocks.is_empty() || !lib_quick::matches(&text) {
            return None;
        }
        if !self.config_bool(LOCAL_COMMANDS_KEY, true).await {
            return None;
        }
        lib_quick::answer(&text, now(), &mut roll)
    }
}

/// Odpowiedź polecenia jako tura agentki (bez dostawcy).
pub(crate) fn run(
    core: &ChatEngine,
    req: &GenRequest,
    handle: &GenHandle,
    reply: Reply,
) -> Outcome {
    let blocks = render_closed(&reply.text);
    if let Ok(mut live) = handle.live.lock() {
        live.text.clone_from(&reply.text);
        live.blocks.clone_from(&blocks);
    }
    if let Some(tap) = &req.tap {
        let _ = tap.send(app_api::ports::VoiceChunk::Text(reply.text.clone()));
    }
    core.emit(AlfaEvent::TextDelta {
        session_id: req.session.to_string(),
        turn_id: ids::turn_dto(&req.session, handle.turn),
        text: reply.text.clone(),
        blocks,
        offset: 0,
    });
    let mut out = Outcome::failed(app_api::dto::TurnError {
        code: app_api::dto::TurnErrorCode::Provider,
        message: String::new(),
        retry_at: None,
        provider: None,
    });
    out.text = reply.text;
    out.status = TurnStatus::Complete;
    out.error = None;
    out.stop = Some(StopReason::End);
    out.local = Some(reply.command.label());
    out
}
