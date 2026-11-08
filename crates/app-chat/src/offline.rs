//! Kolejka offline (wiadomości wysłane bez sieci czekają na `system_retry_queue`): zwrot wpisów,
//! których odpowiedź nie wystartowała (wydzielone z `app-core` przez limit rozmiaru crate'a).

use std::collections::BTreeMap;

use sessions_contract::{SessionId, TurnId};

/// Wpis kolejki offline: sesja i jej czekające tury użytkownika.
pub type QueueEntry = (SessionId, Vec<TurnId>);

/// Zwraca wpisy do kolejki: przed wpisami, które doszły w międzyczasie (kolejność wysyłki).
pub fn requeue(
    queue: &mut BTreeMap<SessionId, Vec<TurnId>>,
    entries: impl IntoIterator<Item = QueueEntry>,
) {
    for (id, mut turns) in entries {
        let entry = queue.entry(id).or_default();
        turns.append(entry);
        *entry = turns;
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
