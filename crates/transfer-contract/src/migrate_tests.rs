use sessions_contract::{AgentId, BranchId};

use super::*;
use crate::manifest::Limits;
use crate::portable::decode_session;

#[test]
fn v0_manifest_is_upcast_and_valid() {
    let raw = serde_json::json!({
        "format": 0,
        "app": "0.0.1",
        "created": "2026-05-01T10:00:00Z",
        "machine": "0123456789abcdef0123456789abcdef",
        "files": [
            { "name": "config/common/shared.toml", "sha256": sha256_hex(b"a = 1\n"), "size": 6 },
            { "name": "sessions/s1/session.json", "sha256": sha256_hex(b"{}"), "size": 2 },
            { "name": "sessions/s1/turns.ndjson", "sha256": sha256_hex(b""), "size": 0 }
        ]
    });
    let (m, steps) = upcast_manifest(raw).unwrap();
    assert_eq!(steps, vec![step("manifest", "0", "1.0.0", 1)]);
    assert_eq!(m.schema_version, schema_version());
    assert_eq!(m.kind, PackageKind::Export);
    assert_eq!(m.scope.keys, vec!["config.common", "sessions"]);
    assert_eq!(m.scope.sessions, vec![SessionId::new("s1")]);
    assert_eq!(m.validate(&Limits::default()), Ok(()));
    let (again, none) = upcast_manifest(serde_json::to_value(&m).unwrap()).unwrap();
    assert_eq!(again, m);
    assert!(none.is_empty());
    assert!(upcast_manifest(serde_json::json!({"format": 0})).is_err());
}

#[test]
fn v0_session_gets_branches_and_authors() {
    let header = br#"{"v":0,"id":"s1","title":"Stara","created":"2026-05-01T10:00:00Z"}"#;
    let turns = concat!(
        "{\"v\":0,\"id\":1,\"parent\":null,\"role\":\"user\",\"text\":\"Pytanie\",\"ts\":\"2026-05-01T10:00:01Z\"}\n",
        "{\"v\":0,\"id\":2,\"parent\":1,\"role\":\"assistant\",\"text\":\"Odp\",\"ts\":\"2026-05-01T10:00:02Z\",\"agent\":\"beta\"}\n",
        "{\"id\":3,\"parent\":1,\"role\":\"assistant\",\"text\":\"Ponów\",\"ts\":\"2026-05-01T10:00:03Z\"}\n",
        "\n",
        "{\"v\":0,\"id\":4,\"parent\":3,\"role\":\"user\",\"text\":\"Dalej\",\"ts\":\"2026-05-01T10:00:04Z\"}\n",
    );
    let (s, steps) = decode_session(&SessionId::new("s1"), header, turns.as_bytes(), None).unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[1].count, 4);
    assert_eq!(s.meta.title, "Stara");
    assert_eq!(s.turns.len(), 4);
    let branches: Vec<BranchId> = s.turns.iter().map(|t| t.branch).collect();
    assert_eq!(
        branches,
        vec![BranchId(1), BranchId(1), BranchId(2), BranchId(2)]
    );
    assert_eq!(
        s.turns[1].author,
        Author::Agent {
            agent: AgentId::new("beta")
        }
    );
    assert_eq!(s.active_leaf, Some(TurnId(4)));
}

#[test]
fn newer_or_broken_records_are_rejected() {
    assert!(matches!(
        decode_turns(b"{\"v\":7}\n", "sessions/s"),
        Err(TransferError::NewerSchema { .. })
    ));
    assert!(matches!(
        decode_turns(b"nie-json\n", "sessions/s"),
        Err(TransferError::Invalid { .. })
    ));
    assert!(matches!(
        upcast_session_header(serde_json::json!({"v": 2}), b"", "s"),
        Err(TransferError::NewerSchema { .. })
    ));
    assert!(decode_turns(&[0xff, 0xfe], "s").is_err());
    let (empty, steps) = decode_turns(b"", "s").unwrap();
    assert!(empty.is_empty() && steps.is_empty());
}
