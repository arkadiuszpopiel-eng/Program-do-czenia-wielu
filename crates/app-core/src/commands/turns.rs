//! Komendy `turns_*`: drzewo tur, wysyłanie (kolejka offline), ponów (wariant), edytuj i wyślij
//! (gałąź), kontynuuj, stop, adnotacje i intencje (pamięć, czytanie, zapis/uruchomienie kodu).

use std::collections::BTreeMap;
use std::time::Duration;

use personas_contract::Personas;
use sessions_contract::{
    Author, Block, NewTurn, Role, SessionCatalog, SessionHistory, SessionId, SessionPatch, TurnId,
};

use crate::core::AppCore;
use crate::dto::{
    AlfaEvent, ModelProfile, SendOptions, SendResult, TurnAnnotation, TurnStatus, TurnsSnapshot,
};
use crate::error::AppError;
use crate::ids;
use crate::settings::keys;
use app_chat::{GenRequest, Placement, author_of, turn_dto};
use app_store::TurnMeta;

impl AppCore {
    /// `turns_list`: całe drzewo (append-only) + tura w trakcie strumienia + adnotacje.
    pub async fn turns_list(&self, session_id: String) -> Result<TurnsSnapshot, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        let turns = self.inner.sessions.all_turns(&id)?;
        let metas = self.inner.store.metas(&id)?;
        let statuses = self.inner.store.statuses(&id)?;
        let ratings = self.inner.store.ratings(&id)?;
        let mut out = Vec::with_capacity(turns.len() + 1);
        let mut annotations = BTreeMap::new();
        for turn in &turns {
            let dto = turn_dto(
                &id,
                turn,
                metas.get(&turn.id.0),
                statuses.get(&turn.id.0).copied(),
            );
            let rating = ratings.get(&turn.id.0).copied();
            if rating.is_some() || turn.hidden {
                annotations.insert(
                    dto.id.clone(),
                    TurnAnnotation {
                        rating,
                        hidden: turn.hidden,
                    },
                );
            }
            out.push(dto);
        }
        if let Some(handle) = self.chat().generation(&id)
            && let Ok(live) = handle.live.lock()
            && !out.iter().any(|t| t.id == live.id)
        {
            out.push(live.clone());
        }
        Ok(TurnsSnapshot {
            turns: out,
            annotations,
        })
    }

    fn system_status_snapshot_online(&self) -> bool {
        self.rt().online
    }

    /// Dopisuje turę użytkownika (+ fakty), opcjonalnie w kolejce offline.
    pub(crate) async fn append_user(
        &self,
        id: &SessionId,
        parent: Option<TurnId>,
        sibling_of: Option<TurnId>,
        text: &str,
        addressed: Option<String>,
        blocks: Vec<Block>,
    ) -> Result<(TurnId, bool), AppError> {
        let text = text.trim();
        if text.is_empty() && blocks.is_empty() {
            return Err(AppError::invalid("Wiadomość jest pusta."));
        }
        let first = self.inner.sessions.turn_count(id)? == 0;
        let mut user = NewTurn::user(text);
        user.content.blocks = blocks;
        let turn = match sibling_of {
            Some(of) => self.inner.sessions.fork_from(id, of, user)?,
            None => self.chat().append_child(id, parent, user)?,
        };
        let meta = TurnMeta {
            status: Some(TurnStatus::Complete),
            addressed_to: addressed,
            ..TurnMeta::default()
        };
        self.inner.store.put_meta(id, turn.id, &meta)?;
        let queued = !self.system_status_snapshot_online();
        if queued {
            self.inner
                .store
                .push_status(id, turn.id, TurnStatus::Queued)?;
            self.rt()
                .queued
                .entry(id.clone())
                .or_default()
                .push(turn.id);
        }
        let dto = turn_dto(id, &turn, Some(&meta), queued.then_some(TurnStatus::Queued));
        self.emit(AlfaEvent::TurnAppended {
            session_id: id.to_string(),
            turn: Box::new(dto),
        });
        if first && !text.is_empty() {
            self.auto_title(id, text).await;
        }
        if queued {
            let status = self.status_snapshot().await;
            self.emit(AlfaEvent::SystemStatusChanged { status });
        }
        Ok((turn.id, queued))
    }

    /// Automatyczny tytuł z pierwszej wiadomości (gdy tytuł jest domyślny szablonu).
    async fn auto_title(&self, id: &SessionId, text: &str) {
        if !self.config_bool(keys::AUTO_TITLE, true).await {
            return;
        }
        let Ok(meta) = self.inner.sessions.session(id) else {
            return;
        };
        if meta.title != sessions_contract::DEFAULT_TITLE {
            return;
        }
        let title: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let title: String = title.chars().take(40).collect();
        let patch = SessionPatch {
            title: Some(title.trim().to_owned()),
            ..SessionPatch::default()
        };
        if self.inner.sessions.update_session(id, patch).is_ok() {
            self.announce_session(id).await;
        }
    }

    /// Adresatka: wskazana w UI albo wynikająca z tekstu (imię) / Dyrygentka obsady.
    pub(crate) fn addressee(&self, id: &SessionId, text: &str, addressed: Option<&str>) -> String {
        match addressed {
            Some(a) => a.to_owned(),
            None => self
                .inner
                .personas
                .resolve_addressee(id, text)
                .as_str()
                .to_owned(),
        }
    }

    /// `turns_send`: tura użytkownika → odpowiedź agentki (offline: tylko kolejka).
    pub async fn turns_send(
        &self,
        session_id: String,
        options: SendOptions,
    ) -> Result<SendResult, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        let parent = match &options.parent_id {
            Some(p) => Some(ids::parse_turn_in(&id, p)?),
            None => None,
        };
        let _guard = self.lock_session(&id).await;
        self.chat().finalize_generation(&id).await;
        let agent = self.addressee(&id, &options.text, options.addressed_to.as_deref());
        let blocks = self.inner.work.files.prepare(&id, &options.attachments)?;
        let (user, queued) = self
            .append_user(
                &id,
                parent,
                None,
                &options.text,
                options.addressed_to.clone(),
                blocks,
            )
            .await?;
        self.inner.work.files.commit(&id, &options.attachments);
        let assistant = if queued {
            None
        } else {
            Some(
                self.chat()
                    .start_generation(GenRequest {
                        session: id.clone(),
                        placement: Placement::Child(user),
                        history_leaf: user,
                        agent,
                        profile: options.profile,
                        continues: None,
                        origin: risk_classifier_contract::CommandOrigin::UserText,
                        tap: None,
                    })
                    .await?,
            )
        };
        Ok(SendResult {
            user_turn_id: ids::turn_dto(&id, user),
            assistant_turn_id: assistant,
        })
    }

    /// Agentka tury (autor albo fakty tury-komunikatu).
    pub(crate) fn agent_of(&self, id: &SessionId, turn: &sessions_contract::Turn) -> String {
        let meta = self.inner.store.meta(id, turn.id).ok().flatten();
        match (
            &turn.author,
            meta.as_ref().and_then(|m| m.addressed_to.clone()),
        ) {
            (Author::User, Some(addressed)) => addressed,
            (Author::User, None) => self.addressee(id, &turn.content.text, None),
            _ => author_of(turn, meta.as_ref()),
        }
    }

    /// `turns_regenerate`: nowy wariant (rodzeństwo) odpowiedzi; `profile` = inny model.
    pub async fn turns_regenerate(
        &self,
        session_id: String,
        turn_id: String,
        profile: Option<String>,
    ) -> Result<String, AppError> {
        let id = ids::session(&session_id)?;
        let target = ids::parse_turn_in(&id, &turn_id)?;
        let _guard = self.lock_session(&id).await;
        self.chat().finalize_generation(&id).await;
        let turn = self.inner.sessions.turn(&id, target)?;
        let (placement, leaf) = match (turn.role, turn.parent) {
            (Role::User, _) => (Placement::Child(target), target),
            (_, Some(parent)) => (Placement::Sibling(target), parent),
            (_, None) => return Err(AppError::invalid("Tura bez wiadomości użytkownika.")),
        };
        let agent = self.agent_of(&id, &turn);
        self.chat()
            .start_generation(GenRequest {
                session: id,
                placement,
                history_leaf: leaf,
                agent,
                profile: profile.as_deref().and_then(ModelProfile::parse),
                continues: None,
                origin: risk_classifier_contract::CommandOrigin::UserText,
                tap: None,
            })
            .await
    }

    /// `turns_edit_and_resend`: nowa gałąź (rodzeństwo tury użytkownika) + odpowiedź.
    pub async fn turns_edit_and_resend(
        &self,
        session_id: String,
        turn_id: String,
        text: String,
    ) -> Result<SendResult, AppError> {
        let id = ids::session(&session_id)?;
        let target = ids::parse_turn_in(&id, &turn_id)?;
        let _guard = self.lock_session(&id).await;
        self.chat().finalize_generation(&id).await;
        let old = self.inner.sessions.turn(&id, target)?;
        if old.role != Role::User {
            return Err(AppError::invalid(
                "Edytować można tylko wiadomość użytkownika.",
            ));
        }
        let addressed = self
            .inner
            .store
            .meta(&id, target)?
            .and_then(|m| m.addressed_to);
        let agent = self.addressee(&id, &text, addressed.as_deref());
        let (user, queued) = self
            .append_user(&id, None, Some(target), &text, addressed, Vec::new())
            .await?;
        let assistant = if queued {
            None
        } else {
            Some(
                self.chat()
                    .start_generation(GenRequest {
                        session: id.clone(),
                        placement: Placement::Child(user),
                        history_leaf: user,
                        agent,
                        profile: None,
                        continues: None,
                        origin: risk_classifier_contract::CommandOrigin::UserText,
                        tap: None,
                    })
                    .await?,
            )
        };
        Ok(SendResult {
            user_turn_id: ids::turn_dto(&id, user),
            assistant_turn_id: assistant,
        })
    }

    /// `turns_continue`: tura-dziecko z `continues = turnId`.
    pub async fn turns_continue(
        &self,
        session_id: String,
        turn_id: String,
    ) -> Result<String, AppError> {
        let id = ids::session(&session_id)?;
        let target = ids::parse_turn_in(&id, &turn_id)?;
        let _guard = self.lock_session(&id).await;
        self.chat().finalize_generation(&id).await;
        let turn = self.inner.sessions.turn(&id, target)?;
        let agent = self.agent_of(&id, &turn);
        self.chat()
            .start_generation(GenRequest {
                session: id,
                placement: Placement::Child(target),
                history_leaf: target,
                agent,
                profile: None,
                continues: Some(target),
                origin: risk_classifier_contract::CommandOrigin::UserText,
                tap: None,
            })
            .await
    }

    /// `turns_stop`: anulowanie ≤ 100 ms → `Stop { reason: cancelled }`.
    pub async fn turns_stop(&self, session_id: String) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        if let Some(handle) = self.chat().generation(&id) {
            handle.cancel.cancel();
            handle.wait(Duration::from_secs(5)).await;
        }
        Ok(())
    }
}
