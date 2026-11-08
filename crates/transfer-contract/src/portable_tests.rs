use std::path::Path;

use chrono::{TimeZone, Utc};
use sessions_contract::{
    Author, BranchId, HeardPrefix, PrivacyTag, Role, SessionTemplate, TurnContent,
};

use super::*;

fn at(s: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_790_000_000 + s, 0)
        .single()
        .unwrap_or_default()
}

pub(crate) fn sample(root: &Path) -> PortableSession {
    let turn = |id: u64, parent: Option<u64>, branch: u64, role: Role, text: &str| Turn {
        id: TurnId(id),
        parent: parent.map(TurnId),
        branch: BranchId(branch),
        role,
        author: if role == Role::User {
            Author::User
        } else {
            Author::Agent {
                agent: sessions_contract::AgentId::new("alfa"),
            }
        },
        content: TurnContent::text(text),
        usage: None,
        created_at: at(id as i64),
        heard_prefix: None,
        hidden: false,
    };
    let mut turns = vec![
        turn(1, None, 1, Role::User, "Cześć"),
        turn(2, Some(1), 1, Role::Assistant, "Dzień dobry, w czym pomóc?"),
        turn(3, Some(1), 2, Role::Assistant, "Wariant"),
    ];
    turns[1].heard_prefix = Some(HeardPrefix {
        chars: 4,
        approximate: true,
    });
    turns[2].hidden = true;
    PortableSession {
        meta: SessionMeta {
            id: SessionId::new("sess-0001"),
            title: "Rozmowa".into(),
            template: SessionTemplate::Coding,
            model_policy: "auto".into(),
            agents: vec![],
            privacy: PrivacyTag::Normal,
            tainted: true,
            workdir: root.join("Rozmowa"),
            pinned: true,
            archived: false,
            trashed: false,
            project: None,
            tags: vec!["a".into()],
            created_at: at(0),
            updated_at: at(9),
        },
        turns,
        active_leaf: Some(TurnId(2)),
        draft: Some("szkic".into()),
    }
}

#[test]
fn round_trip_maps_workdir_to_local_root() {
    let src_root = Path::new("/home/arek/Alfa/Sesje");
    let session = sample(src_root);
    let (header, turns) = encode_session(&session, Some(src_root)).unwrap();
    let text = String::from_utf8(header.clone()).unwrap();
    assert!(!text.contains("arek"), "bez nazwy konta w paczce: {text}");
    assert!(
        String::from_utf8(turns.clone())
            .unwrap()
            .lines()
            .all(|l| l.starts_with("{\"v\":1,"))
    );
    let dst_root = Path::new("/home/inny/Alfa/Sesje");
    let (back, steps) = decode_session(&session.meta.id, &header, &turns, Some(dst_root)).unwrap();
    assert!(steps.is_empty());
    assert_eq!(back.meta.workdir, dst_root.join("Rozmowa"));
    let mut expected = session.clone();
    expected.meta.workdir = dst_root.join("Rozmowa");
    assert_eq!(back, expected);
    // Bez korzenia — ścieżka bez zmian (atrapa).
    let (h2, t2) = encode_session(&session, None).unwrap();
    let (same, _) = decode_session(&session.meta.id, &h2, &t2, None).unwrap();
    assert_eq!(same, session);
}

#[test]
fn rejects_tampering_and_maps_unsafe_workdir() {
    let root = Path::new("/r");
    let session = sample(root);
    let (header, turns) = encode_session(&session, Some(root)).unwrap();
    let mut bad_turns = turns.clone();
    bad_turns.extend_from_slice(b"\n");
    bad_turns[10] ^= 1;
    assert!(matches!(
        decode_session(&session.meta.id, &header, &bad_turns, Some(root)),
        Err(TransferError::Checksum { .. })
    ));
    assert!(matches!(
        decode_session(&SessionId::new("inna"), &header, &turns, Some(root)),
        Err(TransferError::Invalid { .. })
    ));
    let mut h: serde_json::Value = serde_json::from_slice(&header).unwrap();
    h["workdir_rel"] = serde_json::json!("../../Windows/System32");
    let evil = serde_json::to_vec(&h).unwrap();
    let (back, _) = decode_session(&session.meta.id, &evil, &turns, Some(root)).unwrap();
    assert_eq!(back.meta.workdir, root.join("sess-0001"));
    let mut h: serde_json::Value = serde_json::from_slice(&header).unwrap();
    h["meta"]["id"] = serde_json::json!("../x");
    let evil = serde_json::to_vec(&h).unwrap();
    assert!(decode_session(&SessionId::new("../x"), &evil, &turns, Some(root)).is_err());
    assert!(
        encode_turn_line(&session.turns[0])
            .unwrap()
            .starts_with("{\"v\":1,")
    );
    assert_eq!(latest(&session.turns), Some(at(3)));
}
