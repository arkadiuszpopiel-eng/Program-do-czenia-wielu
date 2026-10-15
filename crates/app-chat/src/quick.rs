//! Polecenia bez modelu (P5, `lib-quick`): rachunki, literowanie, losowanie, data i godzina
//! oraz quiz z tabliczki mnożenia (stan w pamięci, jeden na sesję) i własne polecenia
//! użytkownika (`sessions.custom_commands`, pierwszeństwo przed wbudowanymi).
//! Tylko jawna wiadomość użytkownika (tekst albo głos) — nigdy treść agentki, wyzwalacza ani
//! kontynuacja. Odpowiedź liczona lokalnie, natychmiast i także offline (wtedy wiadomość nie
//! trafia do kolejki); bez dostawcy, kosztu i tokenów. Wyłączane ustawieniem
//! `sessions.local_commands`.

use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use lib_quick::custom::{CustomRef, Mode};
use lib_quick::keys::{CUSTOM, ENABLED};
use lib_quick::{Command, Reply, quiz};
use risk_classifier_contract::CommandOrigin;
use sessions_contract::{Role, SessionHistory, SessionId};

use app_api::dto::{AlfaEvent, CustomCommand, QuickMatchMode, StopReason, TurnStatus};
use app_api::ids;

use crate::engine::{ChatEngine, GenHandle};
use crate::generate::GenRequest;
use crate::outcome::Outcome;
use crate::project::render_closed;

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

/// Własne polecenie pasujące do wiadomości (lista z konfiguracji; błędne dane = brak poleceń).
fn custom(value: Option<&serde_json::Value>, text: &str) -> Option<Reply> {
    let list: Vec<CustomCommand> = match value? {
        serde_json::Value::String(s) => serde_json::from_str(s).ok()?,
        v => serde_json::from_value(v.clone()).ok()?,
    };
    let refs: Vec<CustomRef<'_>> = list
        .iter()
        .map(|c| CustomRef {
            phrases: &c.phrases,
            mode: match c.mode {
                QuickMatchMode::Exact => Mode::Exact,
                QuickMatchMode::Starts => Mode::Starts,
                QuickMatchMode::Contains => Mode::Contains,
                QuickMatchMode::Regex => Mode::Regex,
            },
            reply: &c.reply,
            enabled: c.enabled,
            priority: c.priority,
        })
        .collect();
    let (_, text) = lib_quick::custom::find(&refs, text)?;
    Some(Reply {
        command: Command::Custom,
        text,
    })
}

impl ChatEngine {
    /// Czy wiadomość użytkownika dostanie odpowiedź bez modelu (np. offline — bez kolejki):
    /// polecenie, początek quizu albo odpowiedź w trwającym quizie tej sesji.
    pub async fn answers_locally(&self, session: &SessionId, text: &str) -> bool {
        if !self.config_bool(ENABLED, true).await {
            return false;
        }
        let quiz = self.rt().quizzes.get(session).cloned();
        let answer = quiz.is_some_and(|mut q| quiz::reply(&mut q, text, &mut |_| 0).is_some());
        answer
            || custom(self.config_value(CUSTOM).await.as_ref(), text).is_some()
            || lib_quick::matches(text)
            || quiz::start(text, &mut |_| 0).is_some()
    }

    /// Odpowiedź bez modelu: odpowiedź w trwającym quizie sesji, własne polecenie, początek
    /// quizu albo polecenie wbudowane. Wiadomość spoza quizu kończy go bez słowa.
    fn local_reply(
        &self,
        session: &SessionId,
        text: &str,
        commands: Option<&serde_json::Value>,
    ) -> Option<Reply> {
        let mut rt = self.rt();
        if let Some(state) = rt.quizzes.get_mut(session) {
            let step = quiz::reply(state, text, &mut roll);
            if step.as_ref().is_none_or(|(_, done)| *done) {
                rt.quizzes.remove(session);
            }
            if let Some((text, _)) = step {
                return Some(Reply {
                    command: Command::Quiz,
                    text,
                });
            }
        }
        if let Some(reply) = custom(commands, text) {
            return Some(reply);
        }
        if let Some((state, text)) = quiz::start(text, &mut roll) {
            rt.quizzes.insert(session.clone(), state);
            return Some(Reply {
                command: Command::Quiz,
                text,
            });
        }
        drop(rt);
        lib_quick::answer(text, now(), &mut roll)
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
        if !last.content.blocks.is_empty() || !self.config_bool(ENABLED, true).await {
            self.rt().quizzes.remove(&req.session);
            return None;
        }
        let commands = self.config_value(CUSTOM).await;
        self.local_reply(&req.session, &text, commands.as_ref())
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
