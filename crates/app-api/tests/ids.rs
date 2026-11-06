//! Identyfikatory DTO niosą sesję: tury, artefakty, kroki cofania i kroki przebiegu.

#![allow(clippy::unwrap_used)]

use app_api::ids::*;
use artifacts_contract::ArtifactId;
use sessions_contract::{SessionId, TurnId};

#[test]
fn turn_ids_roundtrip_and_reject_garbage() {
    let s = SessionId::new("0192-abc");
    let id = turn_dto(&s, TurnId(12));
    assert_eq!(id, "0192-abc:t12");
    assert_eq!(parse_turn(&id).unwrap(), (s.clone(), TurnId(12)));
    assert_eq!(parse_turn_in(&s, &id).unwrap(), TurnId(12));
    assert!(parse_turn_in(&SessionId::new("inna"), &id).is_err());
    for bad in ["", "q4b", ":t1", "s:x1", "s:t0", "s:t-1", "s:t"] {
        assert!(parse_turn(bad).is_err(), "{bad}");
    }
}

#[test]
fn artifact_ids_and_sessions() {
    let s = SessionId::new("s1");
    let id = artifact_dto(&s, &ArtifactId("a-7:x".into()));
    assert_eq!(parse_artifact(&id).unwrap().1, ArtifactId("a-7:x".into()));
    assert!(parse_artifact("s1:b").is_err());
    assert!(session("a:b").is_err());
    assert!(session("").is_err());
    assert_eq!(session("s-q3").unwrap(), SessionId::new("s-q3"));
    assert!(timeline_dto(&s).starts_with("s1:e"));
}

#[test]
fn undo_tokens_carry_session() {
    let s = SessionId::new("s-1");
    let token = undo_dto(&s, 42);
    assert_eq!(token, "s-1:u42");
    assert_eq!(parse_undo(&token).unwrap(), (s, 42));
    for bad in ["", "s-1", "s-1:x4", ":u1", "s:u", "s:u-1"] {
        assert!(parse_undo(bad).is_err(), "{bad}");
        assert!(parse_any_undo(bad).is_err(), "{bad}");
    }
    let clip = undo_clip_dto(&SessionId::new("s-1"), 7);
    assert_eq!(
        parse_any_undo(&clip).unwrap(),
        (SessionId::new("s-1"), UndoKind::Clipboard, 7)
    );
    assert_eq!(parse_any_undo("s-1:u3").unwrap().1, UndoKind::Journal);
    let env = undo_env_dto(&SessionId::new("s-1"), 4);
    assert_eq!(
        parse_any_undo(&env).unwrap(),
        (SessionId::new("s-1"), UndoKind::System, 4)
    );
    assert!(parse_any_undo("s-1:x4").is_err());
}

#[test]
fn step_ids_carry_session_and_run() {
    let s = SessionId::new("s-1");
    assert_eq!(run_dto(&s, "ab-12"), "s-1:rab-12");
    let id = step_dto(&s, "ab-12", 4);
    assert_eq!(parse_step(&id).unwrap(), (s, "ab-12".to_owned(), 4));
    for bad in [
        "",
        "s-1:rab",
        "s-1:r:s1",
        "s-1:rab:x1",
        ":rab:s1",
        "s:ra:b:s1",
    ] {
        assert!(parse_step(bad).is_err(), "{bad}");
    }
}
