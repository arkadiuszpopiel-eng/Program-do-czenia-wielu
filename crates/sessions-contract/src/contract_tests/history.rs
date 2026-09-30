//! Testy kontraktowe historii (drzewo append-only).

use super::ok;
use crate::api::Sessions;
use crate::error::SessionError;
use crate::ids::{SessionId, TurnId};
use crate::session::NewSession;
use crate::turn::{AttachmentRef, Block, HeardPrefix, NewTurn, Role, TurnContent};

fn session(s: &dyn Sessions) -> SessionId {
    ok(s.create_session(NewSession {
        title: "Historia".into(),
        ..NewSession::default()
    }))
    .id
}

fn ids(turns: &[crate::turn::Turn]) -> Vec<TurnId> {
    turns.iter().map(|t| t.id).collect()
}

/// Dopisywanie na końcu linii i projekcja od korzenia.
pub fn append_and_project(s: &dyn Sessions) {
    let id = session(s);
    assert_eq!(ok(s.active_leaf(&id)), None);
    let u1 = ok(s.append_turn(&id, None, NewTurn::user("Cześć")));
    let a1 = ok(s.append_turn(&id, Some(u1.id), NewTurn::assistant("alfa", "Hej!")));
    let u2 = ok(s.append_turn(&id, Some(a1.id), NewTurn::user("Co słychać?")));
    let a2 = ok(s.append_turn(&id, Some(u2.id), NewTurn::assistant("alfa", "Dobrze.")));
    assert_eq!(u1.parent, None);
    assert_eq!(a2.parent, Some(u2.id));
    assert!(u1.id < a1.id && a1.id < u2.id && u2.id < a2.id);
    assert!(
        [a1.branch, u2.branch, a2.branch]
            .iter()
            .all(|b| *b == u1.branch)
    );
    let proj = ok(s.branch_projection(&id, a2.id));
    assert_eq!(ids(&proj), vec![u1.id, a1.id, u2.id, a2.id]);
    assert_eq!(proj[1], a1);
    assert_eq!(ok(s.active_leaf(&id)), Some(a2.id));
    assert_eq!(ok(s.turn_count(&id)), 4);
    assert_eq!(ok(s.turn(&id, u2.id)), u2);
}

/// Reguły dopisywania: jeden korzeń, tylko do liścia, istniejący rodzic, niepusta treść.
pub fn append_rules(s: &dyn Sessions) {
    let id = session(s);
    let u1 = ok(s.append_turn(&id, None, NewTurn::user("a")));
    ok(s.append_turn(&id, Some(u1.id), NewTurn::assistant("alfa", "b")));
    assert_eq!(
        s.append_turn(&id, None, NewTurn::user("drugi korzeń")),
        Err(SessionError::RootExists)
    );
    assert_eq!(
        s.append_turn(&id, Some(u1.id), NewTurn::user("x")),
        Err(SessionError::NotALeaf { turn: u1.id })
    );
    assert_eq!(
        s.append_turn(&id, Some(TurnId(999)), NewTurn::user("x")),
        Err(SessionError::TurnNotFound { turn: TurnId(999) })
    );
    assert_eq!(
        s.append_turn(&id, Some(u1.id), NewTurn::user(" ")),
        Err(SessionError::EmptyTurn)
    );
    assert_eq!(
        ok(s.turn_count(&id)),
        2,
        "odrzucone dopisania nic nie zapisały"
    );
}

/// „Ponów”/„edytuj” = wariant w nowej gałęzi; stare tury nietknięte.
pub fn fork_creates_variants(s: &dyn Sessions) {
    let id = session(s);
    let u1 = ok(s.append_turn(&id, None, NewTurn::user("Pytanie")));
    let a1 = ok(s.append_turn(&id, Some(u1.id), NewTurn::assistant("alfa", "Odp. 1")));
    let a1b = ok(s.fork_from(&id, a1.id, NewTurn::assistant("beta", "Odp. 2")));
    assert_eq!(a1b.parent, Some(u1.id));
    assert_ne!(a1b.branch, a1.branch);
    assert_eq!(ok(s.active_leaf(&id)), Some(a1b.id));
    let sib = ok(s.siblings(&id, a1.id));
    assert_eq!((sib.turns.clone(), sib.index), (vec![a1.id, a1b.id], 0));
    assert_eq!(ok(s.siblings(&id, a1b.id)).index, 1);
    assert_eq!(
        ids(&ok(s.branch_projection(&id, a1b.id))),
        vec![u1.id, a1b.id]
    );
    assert_eq!(
        ids(&ok(s.branch_projection(&id, a1.id))),
        vec![u1.id, a1.id]
    );
    assert_eq!(ok(s.turn(&id, a1.id)), a1);
    // Kontynuacja starej gałęzi nadal możliwa (a1 jest liściem).
    let u2 = ok(s.append_turn(&id, Some(a1.id), NewTurn::user("Dalej")));
    assert_eq!(u2.branch, a1.branch);
    assert_eq!(ok(s.latest_leaf(&id, u1.id)), a1b.id);
    assert_eq!(ok(s.latest_leaf(&id, a1.id)), u2.id);
    // Edycja pierwszej wiadomości = nowy korzeń (rodzeństwo korzeni).
    let u1b = ok(s.fork_from(&id, u1.id, NewTurn::user("Pytanie inaczej")));
    assert_eq!(u1b.parent, None);
    let roots = ok(s.siblings(&id, u1.id));
    assert_eq!(roots.turns, vec![u1.id, u1b.id]);
    assert_eq!(
        s.fork_from(&id, TurnId(999), NewTurn::user("x")),
        Err(SessionError::TurnNotFound { turn: TurnId(999) })
    );
}

/// Usłyszany prefiks: fakt dopisywany raz, treść tury (assistant_full) bez zmian.
pub fn heard_prefix_is_append_only(s: &dyn Sessions) {
    let id = session(s);
    let u = ok(s.append_turn(&id, None, NewTurn::user("Opowiedz")));
    let a = ok(s.append_turn(
        &id,
        Some(u.id),
        NewTurn::assistant("alfa", "Dzień dobry, słońce świeci"),
    ));
    let prefix = HeardPrefix {
        chars: 5,
        approximate: true,
    };
    let heard = ok(s.record_heard_prefix(&id, a.id, prefix));
    assert_eq!(heard.heard_text(), Some("Dzień"));
    assert_eq!(heard.content, a.content, "assistant_full bez zmian");
    assert_eq!(heard.fingerprint(), a.fingerprint());
    assert_eq!(
        s.record_heard_prefix(&id, a.id, prefix),
        Err(SessionError::HeardPrefixAlreadyRecorded { turn: a.id })
    );
    assert!(matches!(
        s.record_heard_prefix(&id, u.id, prefix),
        Err(SessionError::InvalidHeardPrefix { .. })
    ));
    let mut direct = NewTurn::assistant("alfa", "Krótko");
    direct.heard_prefix = Some(HeardPrefix {
        chars: 6,
        approximate: false,
    });
    let b = ok(s.fork_from(&id, a.id, direct));
    assert_eq!(ok(s.turn(&id, b.id)).heard_prefix, b.heard_prefix);
    assert!(matches!(
        s.record_heard_prefix(
            &id,
            b.id,
            HeardPrefix {
                chars: 99,
                approximate: false
            }
        ),
        Err(SessionError::HeardPrefixAlreadyRecorded { .. })
    ));
}

/// Ukrycie z widoku nie zmienia treści; projekcja zawiera turę z flagą.
pub fn hide_keeps_content(s: &dyn Sessions) {
    let id = session(s);
    let u = ok(s.append_turn(&id, None, NewTurn::user("ukryj mnie")));
    let a = ok(s.append_turn(&id, Some(u.id), NewTurn::assistant("alfa", "ok")));
    ok(s.set_hidden(&id, u.id, true));
    let proj = ok(s.branch_projection(&id, a.id));
    assert!(proj[0].hidden && !proj[1].hidden);
    assert_eq!(proj[0].fingerprint(), u.fingerprint());
    ok(s.set_hidden(&id, u.id, false));
    assert!(!ok(s.turn(&id, u.id)).hidden);
    assert_eq!(
        s.set_hidden(&id, TurnId(999), true),
        Err(SessionError::TurnNotFound { turn: TurnId(999) })
    );
}

/// Szkic composera per sesja.
pub fn drafts_per_session(s: &dyn Sessions) {
    let a = session(s);
    let b = session(s);
    assert_eq!(ok(s.draft(&a)), None);
    ok(s.save_draft(&a, "Szkic „żółć”"));
    assert_eq!(ok(s.draft(&a)), Some("Szkic „żółć”".into()));
    assert_eq!(ok(s.draft(&b)), None);
    ok(s.save_draft(&a, "nowszy"));
    assert_eq!(ok(s.draft(&a)), Some("nowszy".into()));
    ok(s.save_draft(&a, ""));
    assert_eq!(ok(s.draft(&a)), None);
}

/// Bloki IR (myślenie z podpisem, narzędzia, załączniki) wracają bajtowo bez zmian.
pub fn blocks_round_trip(s: &dyn Sessions) {
    let id = session(s);
    let content = TurnContent {
        text: "Wynik gotowy".into(),
        blocks: vec![
            Block::Thinking {
                provider: "anthropic".into(),
                text: "Najpierw sprawdzę plik…".into(),
                signature: Some("EqQBCkYIARgCKkD+/=".into()),
            },
            Block::RedactedThinking {
                provider: "anthropic".into(),
                data: "zaszyfrowane".into(),
            },
            Block::ToolUse {
                id: "tu_1".into(),
                name: "fs.read".into(),
                input: serde_json::json!({"path": "C:\\dane\\a.txt", "n": 1.5}),
            },
            Block::ToolResult {
                tool_use_id: "tu_1".into(),
                content: "zawartość".into(),
                is_error: false,
            },
            Block::Attachment {
                attachment: AttachmentRef {
                    name: "a.png".into(),
                    mime: "image/png".into(),
                    artifact_id: None,
                    sha256: Some("ab".repeat(32)),
                    bytes: Some(10),
                },
            },
            Block::Text {
                text: "Wynik gotowy".into(),
            },
        ],
    };
    let mut new = NewTurn::assistant("alfa", "");
    new.content = content.clone();
    new.role = Role::Assistant;
    let root = ok(s.append_turn(&id, None, new));
    assert_eq!(ok(s.turn(&id, root.id)).content, content);
}

/// Aktywny liść: tylko istniejące tury.
pub fn set_active_leaf_rules(s: &dyn Sessions) {
    let id = session(s);
    let u = ok(s.append_turn(&id, None, NewTurn::user("a")));
    let a = ok(s.append_turn(&id, Some(u.id), NewTurn::assistant("alfa", "b")));
    ok(s.set_active_leaf(&id, u.id));
    assert_eq!(ok(s.active_leaf(&id)), Some(u.id));
    ok(s.set_active_leaf(&id, a.id));
    assert_eq!(ok(s.active_leaf(&id)), Some(a.id));
    assert_eq!(
        s.set_active_leaf(&id, TurnId(42)),
        Err(SessionError::TurnNotFound { turn: TurnId(42) })
    );
    assert_eq!(
        s.branch_projection(&id, TurnId(42)),
        Err(SessionError::TurnNotFound { turn: TurnId(42) })
    );
}
