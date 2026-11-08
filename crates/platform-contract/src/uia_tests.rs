use super::*;

fn node() -> UiaNode {
    UiaNode {
        element: ElementRef::parse("w10:42.1.7").unwrap(),
        depth: 1,
        pid: 7,
        role: control_type_name(50_004).into(),
        name: "Hasło konta".into(),
        automation_id: "pwd".into(),
        class_name: "Edit".into(),
        value: Some("tajne".into()),
        is_password: true,
        enabled: true,
        offscreen: false,
        focused: false,
        toggle: None,
        expand: None,
        selected: None,
        rect: ScreenRect::from_xywh(0, 0, 10, 10),
        patterns: vec![UiaPattern::Value, UiaPattern::Invoke],
    }
}

#[test]
fn element_ref_round_trip_and_rejects_garbage() {
    let r = ElementRef::parse("w123:42.-5.0").unwrap();
    assert_eq!(r.window, WindowId(123));
    assert_eq!(r.runtime_id, vec![42, -5, 0]);
    assert_eq!(r.to_string(), "w123:42.-5.0");
    let text: String = r.clone().into();
    assert_eq!(ElementRef::try_from(text).unwrap(), r);
    for bad in ["", "123:1", "w:1", "wx:1", "w1:", "w1:a", "w1:1..2"] {
        assert!(ElementRef::parse(bad).is_err(), "{bad}");
    }
    let long = format!("w1:{}", vec!["1"; 17].join("."));
    assert!(ElementRef::parse(&long).is_err());
}

#[test]
fn password_value_is_redacted_and_cannot_be_set() {
    let n = node().redacted();
    assert_eq!(n.value, None);
    let set = UiaAction::SetValue { value: "x".into() };
    assert!(matches!(set.check(&n), Err(GuiError::Policy(_))));
    assert!(UiaAction::Invoke.check(&n).is_ok());
    assert!(matches!(
        UiaAction::Toggle.check(&n),
        Err(GuiError::PatternUnsupported(_))
    ));
    let mut off = node();
    off.enabled = false;
    assert!(UiaAction::Invoke.check(&off).is_err());
    let mut plain = node();
    plain.is_password = false;
    let long = UiaAction::SetValue {
        value: "a".repeat(MAX_SET_VALUE_CHARS + 1),
    };
    assert!(long.check(&plain).is_err());
    assert!(set.check(&plain).is_ok());
    assert_eq!(plain.clone().redacted().value.as_deref(), Some("tajne"));
}

#[test]
fn actions_map_to_patterns() {
    let scroll = UiaAction::Scroll {
        direction: ScrollDirection::Down,
        amount: ScrollAmount::Large,
    };
    for (a, p, n) in [
        (UiaAction::Invoke, UiaPattern::Invoke, "invoke"),
        (UiaAction::Toggle, UiaPattern::Toggle, "toggle"),
        (UiaAction::Expand, UiaPattern::ExpandCollapse, "expand"),
        (UiaAction::Collapse, UiaPattern::ExpandCollapse, "collapse"),
        (UiaAction::Select, UiaPattern::SelectionItem, "select"),
        (scroll, UiaPattern::Scroll, "scroll"),
        (
            UiaAction::SetValue { value: "v".into() },
            UiaPattern::Value,
            "set_value",
        ),
    ] {
        assert_eq!(a.pattern(), p);
        assert_eq!(a.name(), n);
    }
}

#[test]
fn query_matching_is_case_insensitive() {
    let n = node();
    let q = UiaQuery {
        name_contains: Some("HASŁO".into()),
        role: Some("Edit".into()),
        max_results: 5,
        ..UiaQuery::default()
    };
    assert!(q.matches(&n) && !q.is_empty());
    let q2 = UiaQuery {
        automation_id: Some("inne".into()),
        ..UiaQuery::default()
    };
    assert!(!q2.matches(&n));
    assert!(UiaQuery::default().is_empty());
    assert_eq!(control_type_name(50_000), "button");
    assert_eq!(control_type_name(50_040), "app_bar");
    assert_eq!(control_type_name(1), "custom");
    let tree = UiaTree {
        window: WindowId(1),
        nodes: vec![n],
        truncated: false,
    };
    assert!(tree.is_sparse());
    assert_eq!(TreeOptions::default().max_nodes, 400);
}
