//! Test property-based `ACC-F1-sessions-02`: dowolna sekwencja operacji daje spójne drzewo,
//! a wcześniej zapisane tury pozostają **bajtowo** niezmienione ([`crate::Turn::fingerprint`]).

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::ok;
use crate::api::Sessions;
use crate::ids::{SessionId, TurnId};
use crate::session::{NewSession, SessionPatch};
use crate::turn::{HeardPrefix, NewTurn, Turn};

/// Operacja na sesji; `pick` wybiera jedną z istniejących tur (modulo liczba tur).
#[derive(Debug, Clone)]
pub enum Op {
    /// Dopisz turę użytkownika/asystentki do liścia pod wybraną turą (albo korzeń, gdy pusto).
    Append {
        /// Wybór tury.
        pick: usize,
        /// Tura asystentki (inaczej użytkownika).
        assistant: bool,
        /// Tekst.
        text: String,
    },
    /// Wariant wybranej tury (nowa gałąź).
    Fork {
        /// Wybór tury.
        pick: usize,
        /// Tekst.
        text: String,
    },
    /// Ukryj/pokaż.
    Hide {
        /// Wybór tury.
        pick: usize,
        /// Ukryta.
        hidden: bool,
    },
    /// Zapisz usłyszany prefiks (błędy dozwolone: tura użytkownika, drugi zapis, za długi).
    Heard {
        /// Wybór tury.
        pick: usize,
        /// Długość.
        chars: usize,
    },
    /// Ustaw aktywny liść.
    SetActive {
        /// Wybór tury.
        pick: usize,
    },
    /// Szkic composera.
    Draft {
        /// Tekst.
        text: String,
    },
    /// Zmiana tytułu i skażenie (metadane nie dotykają historii).
    Meta {
        /// Tytuł.
        title: String,
    },
}

/// Strategia generowania operacji.
pub fn op_strategy() -> impl Strategy<Value = Op> {
    let text = "[a-zA-Ząćęłńóśźż][a-zA-Ząćęłńóśźż ]{0,23}";
    prop_oneof![
        4 => (any::<usize>(), any::<bool>(), text).prop_map(|(pick, assistant, text)| Op::Append {
            pick,
            assistant,
            text
        }),
        2 => (any::<usize>(), text).prop_map(|(pick, text)| Op::Fork { pick, text }),
        1 => (any::<usize>(), any::<bool>()).prop_map(|(pick, hidden)| Op::Hide { pick, hidden }),
        1 => (any::<usize>(), 0_usize..30).prop_map(|(pick, chars)| Op::Heard { pick, chars }),
        1 => any::<usize>().prop_map(|pick| Op::SetActive { pick }),
        1 => "[a-z ]{0,8}".prop_map(|text| Op::Draft { text }),
        1 => "[A-Za-z]{1,8}".prop_map(|title| Op::Meta { title }),
    ]
}

struct Known {
    fingerprint: Vec<u8>,
    heard: Option<HeardPrefix>,
}

/// Wykonuje `ops` na świeżej sesji i po każdym kroku sprawdza niezmienniki; zwraca sesję.
pub fn check_ops(s: &dyn Sessions, ops: &[Op]) -> SessionId {
    let id = ok(s.create_session(NewSession::default())).id;
    check_ops_in(s, &id, ops);
    id
}

/// Jak [`check_ops`], ale na istniejącej sesji `id` (stan wyjściowy = tury `1..=turn_count`,
/// bo identyfikatory tur są kolejnymi liczbami od 1).
pub fn check_ops_in(s: &dyn Sessions, id: &SessionId, ops: &[Op]) {
    let id = id.clone();
    let mut known: BTreeMap<TurnId, Known> = BTreeMap::new();
    for n in 1..=ok(s.turn_count(&id)) {
        let t = ok(s.turn(&id, TurnId(n)));
        known.insert(
            t.id,
            Known {
                fingerprint: t.fingerprint(),
                heard: t.heard_prefix,
            },
        );
    }
    let pick = |known: &BTreeMap<TurnId, Known>, n: usize| -> Option<TurnId> {
        (!known.is_empty())
            .then(|| known.keys().nth(n % known.len()).copied())
            .flatten()
    };
    let remember = |known: &mut BTreeMap<TurnId, Known>, t: &Turn| {
        known.insert(
            t.id,
            Known {
                fingerprint: t.fingerprint(),
                heard: t.heard_prefix,
            },
        );
    };
    for op in ops {
        match op {
            Op::Append {
                pick: n,
                assistant,
                text,
            } => {
                let new = if *assistant {
                    NewTurn::assistant("alfa", text.clone())
                } else {
                    NewTurn::user(text.clone())
                };
                let parent = pick(&known, *n).map(|t| ok(s.latest_leaf(&id, t)));
                let turn = ok(s.append_turn(&id, parent, new));
                remember(&mut known, &turn);
            }
            Op::Fork { pick: n, text } => {
                if let Some(t) = pick(&known, *n) {
                    let turn = ok(s.fork_from(&id, t, NewTurn::user(text.clone())));
                    remember(&mut known, &turn);
                }
            }
            Op::Hide { pick: n, hidden } => {
                if let Some(t) = pick(&known, *n) {
                    ok(s.set_hidden(&id, t, *hidden));
                }
            }
            Op::Heard { pick: n, chars } => {
                if let Some(t) = pick(&known, *n) {
                    let prefix = HeardPrefix {
                        chars: *chars,
                        approximate: true,
                    };
                    if let Ok(turn) = s.record_heard_prefix(&id, t, prefix) {
                        let entry = known.get_mut(&t);
                        assert!(entry.as_ref().is_some_and(|k| k.heard.is_none()));
                        if let Some(k) = entry {
                            k.heard = turn.heard_prefix;
                        }
                    }
                }
            }
            Op::SetActive { pick: n } => {
                if let Some(t) = pick(&known, *n) {
                    ok(s.set_active_leaf(&id, t));
                }
            }
            Op::Draft { text } => ok(s.save_draft(&id, text)),
            Op::Meta { title } => {
                let patch = SessionPatch {
                    title: Some(title.clone()),
                    ..SessionPatch::default()
                };
                ok(s.update_session(&id, patch));
                ok(s.mark_tainted(&id));
            }
        }
        verify(s, &id, &known);
    }
}

fn verify(s: &dyn Sessions, id: &SessionId, known: &BTreeMap<TurnId, Known>) {
    assert_eq!(ok(s.turn_count(id)), known.len() as u64);
    for (turn_id, k) in known {
        let turn = ok(s.turn(id, *turn_id));
        assert_eq!(
            turn.fingerprint(),
            k.fingerprint,
            "tura {turn_id} zmieniona"
        );
        assert_eq!(
            turn.heard_prefix, k.heard,
            "prefiks tury {turn_id} zmieniony"
        );
        let proj = ok(s.branch_projection(id, *turn_id));
        assert_eq!(
            proj.first().map(|t| t.parent),
            Some(None),
            "projekcja od korzenia"
        );
        assert_eq!(proj.last().map(|t| t.id), Some(*turn_id));
        for pair in proj.windows(2) {
            assert_eq!(pair[1].parent, Some(pair[0].id));
        }
        let sib = ok(s.siblings(id, *turn_id));
        assert_eq!(sib.turns.get(sib.index), Some(turn_id));
        assert!(sib.turns.windows(2).all(|w| w[0] < w[1]));
    }
}
