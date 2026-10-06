//! Kolejka offline: wiadomości wysłane bez sieci czekają na `system_retry_queue`.

use sessions_contract::{SessionHistory, SessionId, TurnId};

use crate::core::AppCore;
use crate::dto::{AlfaEvent, SystemStatus, TurnStatus};
use crate::error::AppError;
use crate::ids;
use app_chat::{GenRequest, Placement};

impl AppCore {
    /// Wysyła wiadomości z kolejki offline (po jednej odpowiedzi na sesję — na ostatnią).
    pub(crate) async fn flush_queue(&self) -> Result<(), AppError> {
        let queued: Vec<(SessionId, Vec<TurnId>)> =
            std::mem::take(&mut self.rt().queued).into_iter().collect();
        for (id, turns) in queued {
            let _guard = self.lock_session(&id).await;
            for turn in &turns {
                self.inner
                    .store
                    .push_status(&id, *turn, TurnStatus::Complete)?;
                self.emit(AlfaEvent::TurnStatus {
                    session_id: id.to_string(),
                    turn_id: ids::turn_dto(&id, *turn),
                    status: TurnStatus::Complete,
                });
            }
            let Some(last) = turns.last().copied() else {
                continue;
            };
            self.chat().finalize_generation(&id).await;
            let turn = self.inner.sessions.turn(&id, last)?;
            let agent = self.agent_of(&id, &turn);
            self.chat()
                .start_generation(GenRequest {
                    session: id.clone(),
                    placement: Placement::Child(last),
                    history_leaf: last,
                    agent,
                    profile: None,
                    continues: None,
                    origin: risk_classifier_contract::CommandOrigin::UserText,
                    tap: None,
                })
                .await?;
        }
        Ok(())
    }

    /// Liczba wiadomości w kolejce offline.
    pub(crate) fn queued_count(&self) -> u64 {
        self.rt().queued.values().map(|v| v.len() as u64).sum()
    }

    /// Zmiana łączności zgłaszana przez powłokę / monitor sieci (i testy).
    pub async fn set_online(&self, online: bool) -> SystemStatus {
        self.rt().online = online;
        let status = self.status_snapshot().await;
        self.emit(AlfaEvent::SystemStatusChanged {
            status: status.clone(),
        });
        status
    }
}
