//! Czat rozmowy głosowej (`VoiceChat` dla potoku `voice-pipeline`): wypowiedź użytkownika trafia
//! do aktywnej sesji jako zwykła tura (append-only), odpowiedź agentki strumieniuje się jednocześnie
//! do UI i do syntezy mowy, a po przerwaniu (barge-in) tura dostaje fakt „usłyszany prefiks".

use std::sync::{Arc, Weak};
use std::time::Duration;

use async_trait::async_trait;
use providers_contract::CancellationToken;
use risk_classifier_contract::{CommandOrigin, SttConfidence};
use sessions_contract::{HeardPrefix, SessionHistory, SessionId};
use tokio::sync::mpsc;

use crate::chat::{GenRequest, Placement};
use crate::core::{AppCore, Inner};
use crate::dto::{EventLevel, SessionTemplate, TimelineKind};
use crate::error::AppError;
use crate::ids;
use crate::ports::{KillOrigin, VoiceChat, VoiceTurn, VoiceTurnOrigin, VoiceTurnRef};
use crate::settings::keys;

/// Pewność STT tur głosowych bez pomiaru z potoku (< 800‰ — każda zmiana stanu zlecona głosem
/// pyta nie-głosem); destrukcja głosem zawsze wymaga potwierdzenia nie-głosem (PLAN §6.10).
const VOICE_STT_CONFIDENCE_PERMILLE: u16 = 700;

/// Czat rdzenia dla potoku (słaba referencja — port głosu żyje w rdzeniu).
pub(crate) struct CoreVoiceChat {
    inner: Weak<Inner>,
}

impl CoreVoiceChat {
    pub(crate) fn new(core: &AppCore) -> Self {
        Self {
            inner: Arc::downgrade(&core.inner),
        }
    }

    fn core(&self) -> Result<AppCore, AppError> {
        self.inner
            .upgrade()
            .map(|inner| AppCore { inner })
            .ok_or_else(|| AppError::internal("aplikacja zamyka się"))
    }
}

impl AppCore {
    /// Sesja rozmowy głosowej: aktywna (nieusunięta) albo nowa sesja „Asystent głosowy".
    async fn voice_session(&self) -> Result<SessionId, AppError> {
        if let Some(active) = self.config_str(keys::ACTIVE_SESSION).await
            && let Ok(id) = ids::session(&active)
            && self.ensure_session(&id).is_ok()
        {
            return Ok(id);
        }
        let created = self.sessions_create(SessionTemplate::Voice).await?;
        let id = ids::session(&created.id)?;
        self.app_set_active_session(Some(created.id.clone()))
            .await?;
        self.open_session_in_ui(created.id).await?;
        Ok(id)
    }
}

#[async_trait]
impl VoiceChat for CoreVoiceChat {
    async fn voice_turn(
        &self,
        persona: &str,
        text: &str,
        origin: VoiceTurnOrigin,
        cancel: CancellationToken,
    ) -> Result<VoiceTurn, AppError> {
        let core = self.core()?;
        let session = core.voice_session().await?;
        let _guard = core.lock_session(&session).await;
        core.finalize_generation(&session).await;
        let agent = core.addressee(&session, text, Some(persona));
        let (user, _) = core
            .append_user(&session, None, None, text, Some(agent.clone()), Vec::new())
            .await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let mut req = GenRequest::text(
            session.clone(),
            Placement::Child(user),
            user,
            agent,
            None,
            None,
        );
        // Pochodzenie z potoku (`app-voice`: weryfikacja właściciela F5) — fakty dla Brokera.
        let permille = origin.stt_confidence_permille;
        req.origin = CommandOrigin::UserVoice {
            confidence: SttConfidence::from_permille(
                permille.unwrap_or(VOICE_STT_CONFIDENCE_PERMILLE),
            ),
            speaker_verified: origin.speaker_verified,
        };
        req.tap = Some(tx);
        let id = core.start_generation(req).await?;
        let turn = ids::parse_turn_in(&session, &id)?;
        if let Some(handle) = core.generation(&session).filter(|g| g.turn == turn) {
            tokio::spawn(async move {
                cancel.cancelled().await;
                handle.cancel.cancel();
            });
        }
        Ok(VoiceTurn {
            turn: VoiceTurnRef { session, turn },
            chunks: rx,
        })
    }

    async fn voice_finish(&self, turn: VoiceTurnRef, heard: Option<(String, bool)>) {
        let Ok(core) = self.core() else {
            return;
        };
        if let Some(handle) = core
            .generation(&turn.session)
            .filter(|g| g.turn == turn.turn)
        {
            handle.wait(Duration::from_secs(10)).await;
        }
        let Some((text, approximate)) = heard else {
            return;
        };
        let _guard = core.lock_session(&turn.session).await;
        let prefix = HeardPrefix {
            chars: text.chars().count(),
            approximate,
        };
        match core
            .inner
            .sessions
            .record_heard_prefix(&turn.session, turn.turn, prefix)
        {
            Ok(_) => core.timeline_note(
                &turn.session,
                TimelineKind::Voice,
                EventLevel::Info,
                format!(
                    "Przerwano odpowiedź — usłyszano {} znaków{}",
                    prefix.chars,
                    if approximate {
                        " (w przybliżeniu)"
                    } else {
                        ""
                    }
                ),
                Some(ids::turn_dto(&turn.session, turn.turn)),
            ),
            // Odpowiedź, która nie powstała (tura-komunikat), nie ma prefiksu do zapisania.
            Err(e) => tracing::debug!(error = %e, "usłyszany prefiks pominięty"),
        }
    }

    async fn kill_switch(&self) {
        if let Ok(core) = self.core() {
            core.system_kill_all(KillOrigin::Voice).await;
        }
    }

    async fn cancel_task(&self) {
        let Ok(core) = self.core() else {
            return;
        };
        if let Some(active) = core.config_str(keys::ACTIVE_SESSION).await
            && let Err(e) = core.turns_stop(active).await
        {
            tracing::warn!(error = %e, "anulowanie zadania głosem nie powiodło się");
        }
    }
}
