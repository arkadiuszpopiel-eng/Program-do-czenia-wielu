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
        let mut queued: std::collections::VecDeque<(SessionId, Vec<TurnId>)> =
            std::mem::take(&mut self.rt().queued).into_iter().collect();
        while let Some((id, turns)) = queued.pop_front() {
            if let Err(e) = self.flush_session(&id, &turns).await {
                // Błąd jednej sesji nie może zgubić kolejki: ta sesja i wszystkie nieobsłużone
                // wracają do kolejki (nowe wpisy, które doszły w międzyczasie, zostają na końcu).
                let mut rt = self.rt();
                for (sid, mut rest) in std::iter::once((id, turns)).chain(queued) {
                    let entry = rt.queued.entry(sid).or_default();
                    rest.append(entry);
                    *entry = rest;
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// Jedna sesja z kolejki offline: statusy tur i odpowiedź na ostatnią wiadomość.
    async fn flush_session(&self, id: &SessionId, turns: &[TurnId]) -> Result<(), AppError> {
        {
            let id = id.clone();
            let _guard = self.lock_session(&id).await;
            for turn in turns {
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
                return Ok(());
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
