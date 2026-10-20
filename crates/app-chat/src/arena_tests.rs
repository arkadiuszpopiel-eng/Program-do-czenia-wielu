//! Testy areny: zapis modelu, miejsce odpowiedzi w drzewie, gałęzie (append-only) i wybór gałęzi.

use sessions_contract::{NewSession, NewTurn, SessionCatalog, SessionHistory, SessionId, TurnId};
use sessions_fake::FakeSessions;

use super::{arena_model, arena_placement, select_branch_in};
use crate::generate::Placement;
use crate::history::append_child_in;

fn session(s: &FakeSessions) -> SessionId {
    s.create_session(NewSession {
        title: "Arena".into(),
        ..NewSession::default()
    })
    .unwrap()
    .id
}

#[test]
fn model_must_be_provider_and_model() {
    assert_eq!(
        arena_model(" anthropic:claude-opus-5-5 ").unwrap(),
        "anthropic:claude-opus-5-5"
    );
    assert_eq!(arena_model("local:qwen3-8b").unwrap(), "local:qwen3-8b");
    for bad in [
        "",
        "auto",
        "claude",
        ":model",
        "openai:",
        "Open AI:gpt",
        "openai:gpt 6",
    ] {
        assert!(arena_model(bad).is_err(), "{bad:?} przyjęty");
    }
    assert!(arena_model(&format!("openai:{}", "x".repeat(300))).is_err());
}

#[test]
fn placement_follows_regenerate_rules() {
    let s = FakeSessions::new();
    let id = session(&s);
    let user = s.append_turn(&id, None, NewTurn::user("Pytanie")).unwrap();
    let answer = s
        .append_turn(&id, Some(user.id), NewTurn::assistant("alfa", "Odpowiedź"))
        .unwrap();
    assert_eq!(
        arena_placement(&user).unwrap(),
        (Placement::Child(user.id), user.id)
    );
    assert_eq!(
        arena_placement(&answer).unwrap(),
        (Placement::Sibling(answer.id), user.id)
    );
    let mut orphan = answer.clone();
    orphan.parent = None;
    assert!(arena_placement(&orphan).is_err());
}

/// Dwie odpowiedzi areny pod tą samą wiadomością: nowe gałęzie obok wcześniejszej odpowiedzi,
/// wcześniejsza tura bez zmian, każda gałąź prowadzi przez tę samą wiadomość użytkownika.
#[test]
fn two_answers_become_sibling_branches() {
    let s = FakeSessions::new();
    let id = session(&s);
    let user = s.append_turn(&id, None, NewTurn::user("Porównaj")).unwrap();
    let first = s
        .append_turn(&id, Some(user.id), NewTurn::assistant("alfa", "Pierwsza"))
        .unwrap();
    let a = append_child_in(&s, &id, Some(user.id), NewTurn::assistant("alfa", "A")).unwrap();
    let b = append_child_in(&s, &id, Some(user.id), NewTurn::assistant("alfa", "B")).unwrap();
    let siblings = s.siblings(&id, a.id).unwrap();
    assert_eq!(siblings.turns, vec![first.id, a.id, b.id]);
    assert_ne!(a.branch, first.branch);
    assert_ne!(b.branch, a.branch);
    assert_eq!(
        s.turn(&id, first.id).unwrap(),
        first,
        "historia append-only"
    );
    for leaf in [first.id, a.id, b.id] {
        let path: Vec<TurnId> = s
            .branch_projection(&id, leaf)
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(path, vec![user.id, leaf]);
    }
}

/// „Wybierz tę odpowiedź": aktywny liść = gałąź wybranej; dalsza rozmowa idzie tą gałęzią.
#[test]
fn selecting_branch_moves_active_leaf() {
    let s = FakeSessions::new();
    let id = session(&s);
    let user = s.append_turn(&id, None, NewTurn::user("Porównaj")).unwrap();
    let a = append_child_in(&s, &id, Some(user.id), NewTurn::assistant("alfa", "A")).unwrap();
    let b = append_child_in(&s, &id, Some(user.id), NewTurn::assistant("alfa", "B")).unwrap();
    assert_eq!(s.active_leaf(&id).unwrap(), Some(b.id));
    assert_eq!(select_branch_in(&s, &id, a.id).unwrap(), a.id);
    assert_eq!(s.active_leaf(&id).unwrap(), Some(a.id));
    let next = append_child_in(&s, &id, None, NewTurn::user("Dalej")).unwrap();
    assert_eq!(next.parent, Some(a.id));
    // Wybór tury z dalszym ciągiem → najnowszy liść jej gałęzi.
    assert_eq!(select_branch_in(&s, &id, a.id).unwrap(), next.id);
    assert!(select_branch_in(&s, &id, TurnId(99)).is_err());
}
