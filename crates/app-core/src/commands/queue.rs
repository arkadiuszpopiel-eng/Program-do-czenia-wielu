//! Kolejka offline: wiadomości wysłane bez sieci czekają na `system_retry_queue`.

use std::collections::{BTreeMap, VecDeque};

use sessions_contract::{SessionHistory, SessionId, TurnId};

use crate::core::AppCore;
use crate::dto::{AlfaEvent, SystemStatus, TurnStatus};
use crate::error::AppError;
use crate::ids;
use app_chat::{GenRequest, Placement};

/// Wpis kolejki offline: sesja i jej czekające tury użytkownika.
type Entry = (SessionId, Vec<TurnId>);

/// Zwraca wpisy do kolejki: przed wpisami, które doszły w międzyczasie (kolejność wysyłki).
fn requeue(queue: &mut BTreeMap<SessionId, Vec<TurnId>>, entries: impl IntoIterator<Item = Entry>) {
    for (id, mut turns) in entries {
        let entry = queue.entry(id).or_default();
        turns.append(entry);
        *entry = turns;
    }
}

impl AppCore {
    /// Wysyła wiadomości z kolejki offline (po jednej odpowiedzi na sesję — na ostatnią).
    pub(crate) async fn flush_queue(&self) -> Result<(), AppError> {
        let (mut queued, trashed): (VecDeque<Entry>, Vec<SessionId>) = {
            let mut rt = self.rt();
            let trashed = rt.trash.values().map(|(id, _)| id.clone()).collect();
            (
                std::mem::take(&mut rt.queued).into_iter().collect(),
                trashed,
            )
        };
        while let Some((id, turns)) = queued.pop_front() {
            // Sesja w koszu (trwa okno cofnięcia usunięcia): bez odpowiedzi modelu — wpis czeka
            // na „Cofnij" albo znika razem z sesją (`shred_session`).
            if trashed.contains(&id) {
                requeue(&mut self.rt().queued, [(id, turns)]);
                continue;
            }
            if let Err(e) = self.flush_session(&id, &turns).await {
                // Błąd jednej sesji nie może zgubić kolejki: ta sesja i wszystkie nieobsłużone
                // wracają do kolejki (nowe wpisy, które doszły w międzyczasie, zostają na końcu).
                requeue(
                    &mut self.rt().queued,
                    std::iter::once((id, turns)).chain(queued),
                );
                return Err(e);
            }
        }
        Ok(())
    }

    /// Usuwa wpisy kolejki offline ostatecznie usuwanej sesji.
    pub(crate) fn drop_queued(&self, id: &SessionId) {
        self.rt().queued.remove(id);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// R-01 (docs/STATUS.md): wpisy wracające po błędzie stoją przed tymi, które doszły później,
    /// i żadna tura nie ginie.
    #[test]
    fn requeued_entries_keep_every_turn_in_sending_order() {
        let (a, b, c) = (
            SessionId::new("a"),
            SessionId::new("b"),
            SessionId::new("c"),
        );
        let mut queue = BTreeMap::new();
        queue.insert(a.clone(), vec![TurnId(9)]);
        requeue(
            &mut queue,
            [
                (a.clone(), vec![TurnId(1), TurnId(3)]),
                (b.clone(), vec![TurnId(1)]),
            ],
        );
        assert_eq!(queue[&a], [TurnId(1), TurnId(3), TurnId(9)]);
        assert_eq!(queue[&b], [TurnId(1)]);
        assert!(!queue.contains_key(&c));
        assert_eq!(queue.values().map(Vec::len).sum::<usize>(), 4);
    }
}
