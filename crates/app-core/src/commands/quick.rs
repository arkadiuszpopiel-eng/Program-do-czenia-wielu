//! Komendy `quick_*`: Szybkie pytanie (sesja wg `quick.session_mode`), rozwinięcie do okna
//! głównego, ukrycie okna.

use sessions_contract::{NewSession, SessionHistory};

use crate::core::AppCore;
use crate::dto::{QuickAskResult, SendOptions};
use crate::error::AppError;
use crate::ids;
use crate::settings::keys;

/// Tytuł wspólnej sesji Szybkiego pytania.
pub const QUICK_TITLE: &str = "Szybkie pytania";

impl AppCore {
    async fn quick_session(&self) -> Result<sessions_contract::SessionId, AppError> {
        let single = self.config_str(keys::QUICK_MODE).await.as_deref() != Some("new_each");
        if single
            && let Some(id) = self.config_str(keys::QUICK_SESSION).await
            && let Ok(id) = ids::session(&id)
            && self.ensure_session(&id).is_ok()
        {
            return Ok(id);
        }
        let created = self
            .create_session(NewSession {
                title: QUICK_TITLE.to_owned(),
                agents: vec!["alfa".into()],
                ..NewSession::default()
            })
            .await?;
        let id = ids::session(&created.id)?;
        if single {
            self.config_set(keys::QUICK_SESSION, Some(created.id.into()), false)
                .await?;
        }
        Ok(id)
    }

    /// `quick_ask`.
    pub async fn quick_ask(&self, text: String) -> Result<QuickAskResult, AppError> {
        let id = self.quick_session().await?;
        let parent = self.inner.sessions.active_leaf(&id)?;
        let sent = self
            .turns_send(
                id.to_string(),
                SendOptions {
                    parent_id: parent.map(|p| ids::turn_dto(&id, p)),
                    text,
                    addressed_to: None,
                    profile: None,
                },
            )
            .await?;
        Ok(QuickAskResult {
            session_id: id.to_string(),
            user_turn_id: sent.user_turn_id,
            assistant_turn_id: sent.assistant_turn_id,
        })
    }

    /// `quick_expand_to_main`: aktywna sesja + pokazanie okna głównego.
    pub async fn quick_expand_to_main(&self, session_id: String) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        self.app_set_active_session(Some(id.to_string())).await?;
        self.inner.shell.hide_quick()?;
        self.inner.shell.show_main(Some(id.as_str()))
    }

    /// `quick_hide` (`Esc` w oknie Szybkiego pytania).
    pub async fn quick_hide(&self) -> Result<(), AppError> {
        self.inner.shell.hide_quick()
    }
}
