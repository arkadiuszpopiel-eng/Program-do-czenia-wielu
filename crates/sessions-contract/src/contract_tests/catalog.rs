//! Testy kontraktowe katalogu sesji.

use std::time::Duration;

use super::ok;
use crate::api::Sessions;
use crate::error::SessionError;
use crate::ids::{ProjectId, SessionId};
use crate::naming::DEFAULT_TITLE;
use crate::session::{NewSession, SessionPatch, SessionQuery, SessionSort, SessionTemplate};
use crate::turn::NewTurn;

fn new(title: &str) -> NewSession {
    NewSession {
        title: title.into(),
        ..NewSession::default()
    }
}

fn titles(s: &dyn Sessions, q: &SessionQuery) -> Vec<String> {
    ok(s.list_sessions(q))
        .into_iter()
        .map(|x| x.meta.title)
        .collect()
}

/// Tworzenie i odczyt: normalizacja tytułu, tagów, domyślne wartości.
pub fn create_and_get(s: &dyn Sessions) {
    let meta = ok(s.create_session(NewSession {
        title: "  Żółta łódź ".into(),
        template: SessionTemplate::Coding,
        tags: vec![" b".into(), "a".into(), "a".into()],
        project: Some(ProjectId::new("praca")),
        ..NewSession::default()
    }));
    assert_eq!(meta.title, "Żółta łódź");
    assert_eq!(meta.tags, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(meta.template, SessionTemplate::Coding);
    assert_eq!(meta.model_policy, "auto");
    assert!(!meta.pinned && !meta.archived && !meta.trashed && !meta.tainted);
    assert!(meta.workdir.ends_with("Żółta łódź"));
    assert_eq!(ok(s.session(&meta.id)), meta);
    let untitled = ok(s.create_session(new("   ")));
    assert_eq!(untitled.title, DEFAULT_TITLE);
    assert_ne!(untitled.id, meta.id);
    let missing = SessionId::new("nie-ma-takiej");
    assert_eq!(
        s.session(&missing),
        Err(SessionError::NotFound { id: missing })
    );
}

/// Zmiana metadanych.
pub fn update_meta(s: &dyn Sessions) {
    let meta = ok(s.create_session(new("A")));
    let patched = ok(s.update_session(
        &meta.id,
        SessionPatch {
            title: Some("B".into()),
            pinned: Some(true),
            archived: Some(true),
            project: Some(Some(ProjectId::new("p"))),
            tags: Some(vec!["x".into(), "x".into()]),
            ..SessionPatch::default()
        },
    ));
    assert_eq!(patched.title, "B");
    assert!(patched.pinned && patched.archived);
    assert_eq!(patched.project, Some(ProjectId::new("p")));
    assert_eq!(patched.tags, vec!["x".to_owned()]);
    assert_eq!(
        patched.workdir, meta.workdir,
        "zmiana tytułu nie przenosi katalogu"
    );
    assert!(patched.updated_at >= meta.created_at);
    assert_eq!(ok(s.session(&meta.id)), patched);
    let cleared = ok(s.update_session(
        &meta.id,
        SessionPatch {
            project: Some(None),
            ..SessionPatch::default()
        },
    ));
    assert_eq!(cleared.project, None);
    let missing = SessionId::new("brak");
    assert!(matches!(
        s.update_session(&missing, SessionPatch::default()),
        Err(SessionError::NotFound { .. })
    ));
}

/// Flaga `tainted` tylko rośnie.
pub fn tainted_only_grows(s: &dyn Sessions) {
    let meta = ok(s.create_session(new("T")));
    assert!(ok(s.mark_tainted(&meta.id)).tainted);
    assert!(ok(s.mark_tainted(&meta.id)).tainted);
    let after_patch = ok(s.update_session(&meta.id, SessionPatch::default()));
    assert!(after_patch.tainted);
}

/// Lista: wyszukiwanie bez diakrytyków, filtry, przypięte na górze, porządki.
pub fn list_filters_and_sorts(s: &dyn Sessions) {
    let pause = || std::thread::sleep(Duration::from_millis(3));
    let alfa = ok(s.create_session(new("Alfa")));
    pause();
    let zolc = ok(s.create_session(NewSession {
        tags: vec!["kolor".into()],
        project: Some(ProjectId::new("farby")),
        ..new("Żółć")
    }));
    pause();
    let beta = ok(s.create_session(new("beta")));
    pause();
    ok(s.append_turn(&alfa.id, None, NewTurn::user("najnowsza aktywność")));
    let all = SessionQuery::default();
    assert_eq!(titles(s, &all), vec!["Alfa", "beta", "Żółć"]);
    let by_title = SessionQuery {
        sort: SessionSort::Title,
        ..SessionQuery::default()
    };
    assert_eq!(titles(s, &by_title), vec!["Alfa", "beta", "Żółć"]);
    let created = SessionQuery {
        sort: SessionSort::Created,
        ..SessionQuery::default()
    };
    assert_eq!(titles(s, &created), vec!["beta", "Żółć", "Alfa"]);
    for text in ["zolc", "ŻÓŁ", "ółć"] {
        let q = SessionQuery {
            text: Some(text.into()),
            ..SessionQuery::default()
        };
        assert_eq!(titles(s, &q), vec!["Żółć"], "wyszukiwanie {text}");
    }
    let tagged = SessionQuery {
        tag: Some("kolor".into()),
        ..SessionQuery::default()
    };
    assert_eq!(titles(s, &tagged), vec!["Żółć"]);
    let project = SessionQuery {
        project: Some(ProjectId::new("farby")),
        ..SessionQuery::default()
    };
    assert_eq!(titles(s, &project), vec!["Żółć"]);
    let pin = SessionPatch {
        pinned: Some(true),
        ..SessionPatch::default()
    };
    ok(s.update_session(&zolc.id, pin));
    assert_eq!(titles(s, &all), vec!["Żółć", "Alfa", "beta"]);
    let archive = SessionPatch {
        archived: Some(true),
        ..SessionPatch::default()
    };
    ok(s.update_session(&beta.id, archive));
    assert_eq!(titles(s, &all), vec!["Żółć", "Alfa"]);
    let with_archived = SessionQuery {
        include_archived: true,
        ..SessionQuery::default()
    };
    assert_eq!(titles(s, &with_archived).len(), 3);
    ok(s.trash_session(&alfa.id));
    assert_eq!(titles(s, &all), vec!["Żółć"]);
    let trash = SessionQuery {
        trashed: true,
        ..SessionQuery::default()
    };
    assert_eq!(titles(s, &trash), vec!["Alfa"]);
}

/// Nieprzeczytane (tury inne niż użytkownika) i kropka aktywności.
pub fn unread_and_activity(s: &dyn Sessions) {
    let meta = ok(s.create_session(new("U")));
    let summary = |s: &dyn Sessions| {
        ok(s.list_sessions(&SessionQuery::default()))
            .into_iter()
            .find(|x| x.meta.id == meta.id)
            .unwrap_or_else(|| panic!("brak sesji na liście"))
    };
    let u = ok(s.append_turn(&meta.id, None, NewTurn::user("pytanie")));
    assert_eq!(summary(s).unread, 0);
    ok(s.append_turn(
        &meta.id,
        Some(u.id),
        NewTurn::assistant("alfa", "odpowiedź"),
    ));
    let sum = summary(s);
    assert_eq!((sum.unread, sum.turns), (1, 2));
    assert!(sum.last_turn_at.is_some());
    ok(s.mark_read(&meta.id));
    assert_eq!(summary(s).unread, 0);
    assert!(!summary(s).active);
    ok(s.set_activity(&meta.id, true));
    assert!(summary(s).active);
    ok(s.set_activity(&meta.id, false));
    assert!(!summary(s).active);
}

/// Kosz, przywracanie i ostateczne usunięcie.
pub fn trash_restore_delete(s: &dyn Sessions) {
    let meta = ok(s.create_session(new("Do kosza")));
    ok(s.append_turn(&meta.id, None, NewTurn::user("treść")));
    assert!(ok(s.trash_session(&meta.id)).trashed);
    assert!(!ok(s.restore_session(&meta.id)).trashed);
    let report = ok(s.delete_session(&meta.id));
    assert!(report.key_deleted);
    let gone = SessionError::NotFound {
        id: meta.id.clone(),
    };
    assert_eq!(s.session(&meta.id), Err(gone.clone()));
    assert_eq!(s.turn_count(&meta.id), Err(gone.clone()));
    assert_eq!(s.delete_session(&meta.id), Err(gone));
    assert!(ok(s.list_sessions(&SessionQuery::default())).is_empty());
}

/// Zero przecieków: tury sesji A nie istnieją w sesji B.
pub fn isolation_between_sessions(s: &dyn Sessions) {
    let a = ok(s.create_session(new("A")));
    let b = ok(s.create_session(new("B")));
    let t = ok(s.append_turn(&a.id, None, NewTurn::user("sekret sesji A")));
    assert_eq!(ok(s.turn_count(&b.id)), 0);
    assert_eq!(
        s.turn(&b.id, t.id),
        Err(SessionError::TurnNotFound { turn: t.id })
    );
    assert_eq!(ok(s.active_leaf(&b.id)), None);
    ok(s.save_draft(&a.id, "szkic A"));
    assert_eq!(ok(s.draft(&b.id)), None);
}

/// Katalogi robocze sesji o tym samym tytule są różne.
pub fn workdirs_are_unique(s: &dyn Sessions) {
    let first = ok(s.create_session(new("Raport")));
    let second = ok(s.create_session(new("Raport")));
    assert_ne!(first.workdir, second.workdir);
    assert!(second.workdir.ends_with("Raport (2)"));
}
