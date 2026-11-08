use platform_contract::{ScreenRect, UiaNode, WindowId};

use super::*;

fn node(password: bool) -> UiaNode {
    UiaNode {
        element: platform_contract::ElementRef {
            window: WindowId(5),
            runtime_id: vec![42, 5, 1],
        },
        depth: 2,
        pid: 1,
        role: "edit".into(),
        name: "Token sk-ant-api03-ABCDEFGHIJKLMNOPQRSTUV".into(),
        automation_id: "tok".into(),
        class_name: "Edit".into(),
        value: Some("password=hunter2".into()),
        is_password: password,
        enabled: true,
        offscreen: false,
        focused: true,
        toggle: None,
        expand: None,
        selected: None,
        rect: ScreenRect::from_xywh(1, 2, 3, 4),
        patterns: vec![platform_contract::UiaPattern::Value],
    }
}

#[test]
fn manifests_args_and_conversions() {
    for m in manifests() {
        m.validate().unwrap();
        assert_eq!(
            check_args(&m.name, &sample_args(&m.name)),
            Ok(()),
            "{}",
            m.name
        );
        assert!(check_args(&m.name, &serde_json::json!({"zzz": 1})).is_err());
        assert_eq!(
            m.reversible,
            if m.mutating {
                Reversibility::No
            } else {
                Reversibility::Yes
            }
        );
    }
    assert!(
        check_args("uia_find", &serde_json::json!({"window": 1})).is_err(),
        "bez kryteriów"
    );
    assert!(
        check_args(
            "uia_act",
            &serde_json::json!({"element": "zly", "action": "invoke"})
        )
        .is_err()
    );
    assert!(
        check_args(
            "uia_act",
            &serde_json::json!({"element": "w1:1", "action": "scroll"})
        )
        .is_err()
    );
    assert!(
        check_args(
            "uia_act",
            &serde_json::json!({"element": "w1:1", "action": "invoke", "value": "x"})
        )
        .is_err()
    );
    assert!(
        check_args(
            "uia_act",
            &serde_json::json!({"element": "w1:1", "action": "scroll", "direction": "down"})
        )
        .is_ok()
    );
    assert!(check_args("nope", &serde_json::json!({})).is_err());
    let q = to_query(
        &serde_json::from_value(
            serde_json::json!({"window": 1, "role": "button", "max_results": 900}),
        )
        .unwrap(),
        50,
    )
    .unwrap();
    assert_eq!(q.max_results, 50);
}

#[test]
fn outputs_redact_secrets_and_passwords() {
    let out = node_out(&node(false));
    assert!(!out.name.contains("sk-ant") && out.value.as_deref() == Some("password=[ZREDAGOWANO]"));
    assert_eq!(out.patterns, vec!["set_value".to_owned()]);
    let pw = node_out(&node(true));
    assert_eq!(pw.value, None);
    let text = render_nodes(&[pw, out]);
    assert!(text.contains("(pole hasła)") && !text.contains("hunter2"));
    assert!(text.contains("[w5:42.5.1]"));
    assert_eq!(UiaToolsConfig::default().max_nodes, 300);
}
