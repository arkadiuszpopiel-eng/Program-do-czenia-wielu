use serde_json::json;

use super::*;

#[test]
fn first_line_is_parsed_and_must_be_loopback() {
    let l =
        Listening::parse(r#"{"listening":"ws://127.0.0.1:50123","mode":"headless","paused":true}"#)
            .unwrap();
    assert_eq!(l.host_port().unwrap(), "127.0.0.1:50123");
    assert!(l.paused);
    let v6 =
        Listening::parse(r#"{"listening":"ws://[::1]:9","mode":"gpu_preview","paused":false}"#);
    assert_eq!(v6.unwrap().host_port().unwrap(), "[::1]:9");
    for bad in [
        r#"{"listening":"ws://0.0.0.0:1","mode":"headless","paused":true}"#,
        r#"{"listening":"ws://192.168.1.2:1","mode":"headless","paused":true}"#,
        r#"{"listening":"ws://127.0.0.1:0","mode":"headless","paused":true}"#,
        r#"{"listening":"ws://127.0.0.1:1/x","mode":"headless","paused":true}"#,
        r#"{"listening":"ws://u@127.0.0.1:1","mode":"headless","paused":true}"#,
        r#"{"listening":"http://127.0.0.1:1","mode":"headless","paused":true}"#,
        "engine-server: address in use",
    ] {
        assert!(Listening::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn request_and_response_follow_json_rpc() {
    let text = request(7, EngineMethod::HeadlessStep, &json!({"frames": 2})).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["method"], "engine.headless_step");
    assert_eq!(v["id"], 7);
    assert_eq!(v["jsonrpc"], "2.0");
    let ok = response(7, r#"{"jsonrpc":"2.0","id":7,"result":{"frame":2}}"#).unwrap();
    assert_eq!(ok, Some(json!({"frame": 2})));
    assert_eq!(
        response(7, r#"{"jsonrpc":"2.0","id":6,"result":{}}"#).unwrap(),
        None
    );
    let err = response(
        7,
        r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32000,"message":"stale","data":{"code":"revision_conflict"}}}"#,
    )
    .unwrap_err();
    assert_eq!(
        err,
        EngineError::Rpc {
            code: -32000,
            kind: "revision_conflict".into(),
            message: "stale".into()
        }
    );
    let parse = response(
        7,
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error","data":null}}"#,
    );
    assert!(matches!(parse, Err(EngineError::Rpc { code: -32700, .. })));
    assert!(response(7, "nie json").is_err());
    assert!(response(7, r#"{"id":7,"result":{}}"#).is_err());
    assert!(response(7, r#"{"jsonrpc":"2.0","id":7}"#).is_err());
}

#[test]
fn oversized_requests_are_refused_before_sending() {
    let big = "x".repeat(MAX_MESSAGE_BYTES);
    let err = request(1, EngineMethod::HotReloadShader, &json!({"wgsl_code": big})).unwrap_err();
    assert!(matches!(err, EngineError::TooLarge { .. }));
}

#[test]
fn methods_are_classified_and_named() {
    for m in [
        EngineMethod::GetSpatialSelection,
        EngineMethod::InspectSceneTelemetry,
        EngineMethod::InspectWorldStreaming,
        EngineMethod::RaycastWorld,
        EngineMethod::QueryWorldHeight,
    ] {
        assert!(!m.mutating(), "{m:?}");
        assert!(m.wire().starts_with("engine."));
    }
    for m in [
        EngineMethod::ExecuteTransaction,
        EngineMethod::RollbackLastChange,
        EngineMethod::HeadlessStep,
        EngineMethod::SetSpatialSelection,
        EngineMethod::HotReloadShader,
        EngineMethod::StreamWorld,
    ] {
        assert!(m.mutating(), "{m:?}");
    }
    assert!(hint("revision_conflict").contains("expected_revision"));
    assert!(!hint("coś-nowego").is_empty());
}

#[test]
fn manifests_are_consistent_with_access() {
    let all = manifests();
    assert_eq!(all.len(), 15);
    let mut names: Vec<_> = all.iter().map(|m| m.name.clone()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 15, "nazwy unikalne");
    for m in &all {
        assert!(m.name.starts_with("engine_"));
        assert!(m.groups.contains(&"engine".to_owned()));
        let expected = match access(&m.name) {
            Access::Read => ("fs.read", false),
            Access::Process => ("shell.exec", true),
            Access::Query => ("gui.control", false),
            Access::Change => ("gui.control", true),
        };
        assert_eq!(
            (m.capabilities[0].as_str(), m.mutating),
            expected,
            "{}",
            m.name
        );
        assert_eq!(
            check_args(&m.name, &sample_args(&m.name)),
            Ok(()),
            "{}",
            m.name
        );
    }
    assert!(check_args("engine_x", &json!({})).is_err());
}

#[test]
fn argument_ranges_match_the_server() {
    assert!(check_args("engine_step", &json!({"frames": 10_000})).is_ok());
    assert!(check_args("engine_step", &json!({"frames": 10_001})).is_err());
    assert!(check_args("engine_stream", &json!({"budget": 33})).is_err());
    assert!(check_args("engine_inspect", &json!({"radius_meters": 10_000.5})).is_err());
    assert!(check_args("engine_inspect", &json!({"what": "selection"})).is_ok());
    assert!(check_args("engine_preview", &json!({"width": 4096})).is_err());
    assert!(
        check_args(
            "engine_raycast",
            &json!({"origin": [0, 0, 0], "direction": [0, 1, 0], "max_distance": -1})
        )
        .is_err()
    );
    assert!(check_args("engine_height", &json!({"x": 1, "z": 2, "y": 3})).is_err());
    assert!(check_transaction_id(&"a".repeat(128)).is_ok());
    assert!(check_transaction_id(&"a".repeat(129)).is_err());
    assert!(check_transaction_id("a\nb").is_err());
    let ops: Vec<_> = (0..1025)
        .map(|i| json!({"op": "entity.despawn", "target_id": i}))
        .collect();
    assert!(check_args("engine_transaction", &json!({"operations": ops})).is_err());
    assert!(
        check_args(
            "engine_start",
            &json!({"load": "świat.json", "paused": false})
        )
        .is_ok()
    );
}
