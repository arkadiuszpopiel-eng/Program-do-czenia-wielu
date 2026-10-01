//! Testy kontraktowe operacji importu/eksportu przenośnego (`all_turns`, `adopt_session`,
//! `import_turns`) używanych przez moduł `transfer`.

use super::ok;
use crate::api::Sessions;
use crate::error::SessionError;
use crate::ids::{BranchId, SessionId, TurnId};
use crate::import::PortableSession;
use crate::session::{NewSession, SessionMeta, SessionQuery};
use crate::turn::{HeardPrefix, NewTurn, Turn};

/// Sesja z gałęziami, drugim korzeniem, prefiksem, ukrytą turą i szkicem; zwraca `(id, liść)`.
fn branched(s: &dyn Sessions) -> (SessionId, TurnId) {
    let id = ok(s.create_session(NewSession {
        title: "Źródło".into(),
        tags: vec!["import".into()],
        ..NewSession::default()
    }))
    .id;
    let u1 = ok(s.append_turn(&id, None, NewTurn::user("Pytanie")));
    let mut full = NewTurn::assistant("alfa", "Długa odpowiedź przerwana w połowie.");
    full.heard_prefix = Some(HeardPrefix {
        chars: 5,
        approximate: true,
    });
    let a1 = ok(s.append_turn(&id, Some(u1.id), full));
    let u2 = ok(s.append_turn(&id, Some(a1.id), NewTurn::user("Stop")));
    ok(s.fork_from(&id, a1.id, NewTurn::assistant("beta", "Wariant")));
    ok(s.fork_from(&id, u1.id, NewTurn::user("Inny początek")));
    let a2 = ok(s.append_turn(&id, Some(u2.id), NewTurn::assistant("alfa", "Dobrze")));
    ok(s.set_hidden(&id, u2.id, true));
    ok(s.save_draft(&id, "szkic"));
    ok(s.mark_tainted(&id));
    (id, a2.id)
}

fn copy_meta(meta: &SessionMeta, id: &str) -> SessionMeta {
    let mut copy = meta.clone();
    copy.id = SessionId::new(id);
    copy
}

fn portable(meta: SessionMeta, turns: Vec<Turn>) -> PortableSession {
    PortableSession {
        meta,
        turns,
        active_leaf: None,
        draft: None,
    }
}

/// `all_turns` zwraca całe drzewo; `adopt_session` + `import_turns` odtwarzają je bajtowo.
pub fn adopt_and_import_round_trip(s: &dyn Sessions) {
    let (src, leaf) = branched(s);
    let turns = ok(s.all_turns(&src));
    assert_eq!(turns.len() as u64, ok(s.turn_count(&src)));
    assert!(turns.windows(2).all(|w| w[0].id < w[1].id));
    assert!(turns.iter().any(|t| t.hidden));
    assert!(turns.iter().any(|t| t.heard_prefix.is_some()));
    assert!(turns.iter().filter(|t| t.parent.is_none()).count() == 2);

    let meta = ok(s.session(&src));
    // Pusta sesja (bez tur) + dopisanie historii przez `import_turns`.
    let empty = ok(s.adopt_session(portable(copy_meta(&meta, "import-0001"), vec![])));
    assert_eq!(empty.id, SessionId::new("import-0001"));
    assert_eq!(empty.created_at, meta.created_at);
    assert_eq!(empty.title, meta.title);
    assert!(empty.tainted, "flaga tainted przenoszona");
    assert_eq!(ok(s.turn_count(&empty.id)), 0);
    assert_eq!(ok(s.import_turns(&empty.id, &turns)), turns.len() as u64);
    assert_eq!(ok(s.all_turns(&empty.id)), turns);
    assert_eq!(
        ok(s.active_leaf(&empty.id)),
        None,
        "import nie zmienia liścia"
    );

    // Cała sesja jednym atomowym wywołaniem (z liściem i szkicem).
    let whole = PortableSession {
        meta: copy_meta(&meta, "import-0003"),
        turns: turns.clone(),
        active_leaf: Some(leaf),
        draft: Some("szkic".into()),
    };
    let adopted = ok(s.adopt_session(whole));
    assert_eq!(ok(s.all_turns(&adopted.id)), turns);
    for t in &turns {
        assert_eq!(ok(s.turn(&adopted.id, t.id)).fingerprint(), t.fingerprint());
    }
    assert_eq!(ok(s.active_leaf(&adopted.id)), Some(leaf));
    assert_eq!(ok(s.draft(&adopted.id)), Some("szkic".into()));
    assert_eq!(
        ok(s.branch_projection(&adopted.id, leaf)),
        ok(s.branch_projection(&src, leaf))
    );
    let list = ok(s.list_sessions(&SessionQuery::default()));
    let summary = list.iter().find(|x| x.meta.id == adopted.id);
    assert_eq!(summary.map(|x| x.turns), Some(turns.len() as u64));
    assert_eq!(
        summary.and_then(|x| x.last_turn_at),
        turns.iter().map(|t| t.created_at).max()
    );
    // Źródło nietknięte.
    assert_eq!(ok(s.all_turns(&src)), turns);
}

/// Istniejący identyfikator → `AlreadyExists`; identyfikator niebezpieczny → `Invalid`.
pub fn adopt_rejects_existing_and_unsafe_ids(s: &dyn Sessions) {
    let (src, _) = branched(s);
    let meta = ok(s.session(&src));
    let turns = ok(s.all_turns(&src));
    assert_eq!(
        s.adopt_session(portable(meta.clone(), vec![])),
        Err(SessionError::AlreadyExists { id: src.clone() })
    );
    for bad in ["../evil", "a/b", "C:\\x", ""] {
        assert!(
            matches!(
                s.adopt_session(portable(copy_meta(&meta, bad), vec![])),
                Err(SessionError::Invalid { .. })
            ),
            "{bad}"
        );
    }
    // Niespójne drzewo albo liść spoza tur → nic nie powstaje (atomowo).
    let mut broken = turns.clone();
    broken[2].branch = BranchId(9);
    assert!(
        s.adopt_session(portable(copy_meta(&meta, "zla-1"), broken))
            .is_err()
    );
    let mut bad_leaf = portable(copy_meta(&meta, "zla-2"), turns.clone());
    bad_leaf.active_leaf = Some(TurnId(999));
    assert!(s.adopt_session(bad_leaf).is_err());
    for id in ["zla-1", "zla-2"] {
        assert!(matches!(
            s.session(&SessionId::new(id)),
            Err(SessionError::NotFound { .. })
        ));
    }
    assert!(matches!(
        s.import_turns(&SessionId::new("brak-sesji"), &ok(s.all_turns(&src))),
        Err(SessionError::NotFound { .. })
    ));
}

fn shifted(t: &Turn, id: u64) -> Turn {
    let mut t = t.clone();
    t.id = TurnId(id);
    t
}

/// Import waliduje reguły drzewa i jest „wszystko albo nic”; dopisanie do istniejącej sesji
/// (scalanie) nie zmienia wcześniejszych tur.
pub fn import_turns_validated_and_atomic(s: &dyn Sessions) {
    let (src, _) = branched(s);
    let turns = ok(s.all_turns(&src));
    let meta = ok(s.session(&src));
    let dst = ok(s.adopt_session(portable(copy_meta(&meta, "import-0002"), vec![]))).id;

    // Luka w numeracji w środku partii → nic nie zapisano.
    let mut gap = turns.clone();
    gap[3] = shifted(&gap[3], 99);
    assert!(s.import_turns(&dst, &gap).is_err());
    assert_eq!(ok(s.turn_count(&dst)), 0);
    // Zła gałąź.
    let mut wrong = turns.clone();
    wrong[1].branch = BranchId(7);
    assert!(s.import_turns(&dst, &wrong).is_err());
    assert_eq!(ok(s.turn_count(&dst)), 0);
    // Pusta treść.
    let mut empty = turns.clone();
    empty[0].content = crate::turn::TurnContent::default();
    assert_eq!(s.import_turns(&dst, &empty), Err(SessionError::EmptyTurn));
    assert_eq!(ok(s.import_turns(&dst, &[])), 0);

    // Najpierw połowa, potem reszta (scalanie „fast-forward”).
    let (head, tail) = turns.split_at(3);
    assert_eq!(ok(s.import_turns(&dst, head)), 3);
    let before: Vec<Vec<u8>> = ok(s.all_turns(&dst))
        .iter()
        .map(Turn::fingerprint)
        .collect();
    assert!(s.import_turns(&dst, head).is_err(), "tury już istnieją");
    assert_eq!(ok(s.import_turns(&dst, tail)), tail.len() as u64);
    let after = ok(s.all_turns(&dst));
    assert_eq!(after, turns);
    let prefix: Vec<Vec<u8>> = after[..3].iter().map(Turn::fingerprint).collect();
    assert_eq!(prefix, before);
    // Po imporcie zwykłe dopisywanie działa dalej i numeruje kolejno.
    let leaf = ok(s.latest_leaf(&dst, TurnId(1)));
    let next = ok(s.append_turn(&dst, Some(leaf), NewTurn::user("dalej")));
    assert_eq!(next.id, TurnId(turns.len() as u64 + 1));
    let variant = ok(s.fork_from(&dst, next.id, NewTurn::user("wariant")));
    let max_branch = turns.iter().map(|t| t.branch).max().unwrap_or(BranchId(0));
    assert_eq!(variant.branch, BranchId(max_branch.0 + 1));
}
