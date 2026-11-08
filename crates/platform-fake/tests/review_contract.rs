//! Przegląd bezpieczeństwa #2 — testy logiki kontraktu `platform-contract` dodanej w poprawkach
//! (tu, a nie w `src/`, bo crate kontraktu jest na granicy limitu 8 000 linii): procesy powiązane
//! z oknem-celem (P2-01), spójność zrzutu z listą okien (P2-02), pole z fokusem (P2-03).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use platform_contract::{
    DesktopWindow, ElementRef, FocusedField, GuiError, InputPacing, InputPlan, InputStep, KeyChord,
    LinkRole, MAX_ANCESTORS, MaskReason, MouseButton, ProcessLink, ScreenRect, TargetGuard,
    UiaNode, WindowId, WindowState, ancestors_of, batch_writes_text, capture_set_stable,
    plan_batches, union_for_mask, unstable_masks,
};

fn link(role: LinkRole, pid: u32, image: &str, ancestors: &[u32]) -> ProcessLink {
    ProcessLink {
        role,
        pid,
        image: image.into(),
        ancestors: ancestors.to_vec(),
    }
}

#[test]
fn ancestors_stop_on_cycles_and_limits() {
    let parents = |p: u32| match p {
        10 => Some(5),
        5 => Some(1),
        1 => Some(0),
        20 => Some(21),
        21 => Some(20),
        p if p >= 100 => Some(p + 1),
        _ => None,
    };
    assert_eq!(ancestors_of(10, parents), vec![5, 1]);
    assert_eq!(ancestors_of(20, parents), vec![21]);
    assert_eq!(ancestors_of(100, parents).len(), MAX_ANCESTORS);
    assert!(ancestors_of(7, parents).is_empty());
}

#[test]
fn descendants_of_alfa_owners_and_unresolved_uwp_are_protected() {
    let me = std::process::id();
    let g = TargetGuard::baseline().with_pids([4242]);
    let webview = r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application\msedgewebview2.exe";
    let popup = [link(LinkRole::Root, 77, webview, &[76, me, 1])];
    assert!(g.is_protected_window(77, webview, &popup));
    assert!(g.check_window(77, webview, &popup, "klik").is_err());
    let other = [link(LinkRole::Root, 88, webview, &[87, 4242])];
    assert!(
        g.is_protected_window(88, webview, &other),
        "potomek Broker-UI"
    );
    let teams = [link(LinkRole::Root, 99, webview, &[98, 3])];
    assert!(!g.is_protected_window(99, webview, &teams));
    assert!(g.check_window(99, webview, &teams, "klik").is_ok());
    let owned = [
        link(LinkRole::Root, 50, "notepad.exe", &[3]),
        link(LinkRole::Owner, 51, "alfa-desktop.exe", &[3]),
    ];
    assert!(g.is_protected_window(50, "notepad.exe", &owned));
    let err = g
        .check_window(50, "notepad.exe", &owned, "klik")
        .unwrap_err();
    assert!(matches!(err, GuiError::ProtectedTarget(m) if m.contains("alfa-desktop.exe")));
    let host = r"C:\Windows\System32\ApplicationFrameHost.exe";
    let uwp_ok = [
        link(LinkRole::Root, 60, host, &[3]),
        link(LinkRole::UwpContent, 61, r"C:\Apps\Calc.exe", &[3]),
    ];
    assert!(!g.is_protected_window(61, r"C:\Apps\Calc.exe", &uwp_ok));
    let uwp_unknown = [
        link(LinkRole::Root, 60, host, &[3]),
        link(LinkRole::UwpUnresolved, 0, "", &[]),
    ];
    assert!(g.is_protected_window(60, host, &uwp_unknown));
    assert!(g.is_protected_window(60, host, &[]), "sama ramka UWP");
    assert!(g.check_window(60, host, &[], "klik").is_err());
    assert!(!g.is_protected_root(0) && g.is_protected_root(me));
}

fn window(id: u64, rect: ScreenRect, protected: bool) -> DesktopWindow {
    DesktopWindow {
        id: WindowId(id),
        title: String::new(),
        class_name: String::new(),
        pid: 1,
        image: "x.exe".into(),
        rect,
        monitor: 0,
        dpi: 96,
        state: WindowState::Normal,
        focused: false,
        z_order: 0,
        elevated: false,
        protected,
    }
}

#[test]
fn capture_set_detects_appearing_and_moving_windows() {
    let src = ScreenRect::from_xywh(0, 0, 100, 100);
    let a = window(1, ScreenRect::from_xywh(0, 0, 50, 50), false);
    let broker = window(2, ScreenRect::from_xywh(10, 10, 20, 20), true);
    let far = window(3, ScreenRect::from_xywh(500, 500, 10, 10), true);
    let before = vec![a.clone()];
    assert!(capture_set_stable(&src, &before, &[a.clone(), far]));
    assert!(!capture_set_stable(
        &src,
        &before,
        &[a.clone(), broker.clone()]
    ));
    let mut moved = a.clone();
    moved.rect = ScreenRect::from_xywh(40, 40, 50, 50);
    assert!(!capture_set_stable(
        &src,
        &before,
        std::slice::from_ref(&moved)
    ));
    let mut z = a.clone();
    z.z_order = 7;
    assert!(
        capture_set_stable(&src, &before, &[z]),
        "kolejność Z bez znaczenia"
    );
    let union = union_for_mask(&[broker.clone(), a.clone()], std::slice::from_ref(&moved));
    assert_eq!(union.len(), 3);
    assert!(union.iter().any(|x| x.id == broker.id && x.protected));
    let extra = unstable_masks(&src, std::slice::from_ref(&a), &[moved, broker]);
    assert_eq!(extra.len(), 3, "stara i nowa pozycja + nowe okno");
    assert!(extra.iter().all(|m| m.reason == MaskReason::Unverified));
    assert!(unstable_masks(&src, &before, &before).is_empty());
}

fn node(password: bool) -> UiaNode {
    UiaNode {
        element: ElementRef {
            window: WindowId(1),
            runtime_id: vec![1],
        },
        depth: 0,
        pid: 1,
        role: "edit".into(),
        name: "Pole".into(),
        automation_id: String::new(),
        class_name: String::new(),
        value: None,
        is_password: password,
        enabled: true,
        offscreen: false,
        focused: true,
        toggle: None,
        expand: None,
        selected: None,
        rect: ScreenRect::default(),
        patterns: Vec::new(),
    }
}

#[test]
fn focused_field_lookup_is_fail_closed() {
    assert_eq!(
        FocusedField::from_lookup(&Ok(Some(node(false)))),
        FocusedField::Ordinary
    );
    assert_eq!(
        FocusedField::from_lookup(&Ok(Some(node(true)))),
        FocusedField::Password
    );
    assert_eq!(FocusedField::from_lookup(&Ok(None)), FocusedField::Unknown);
    let err = Err(GuiError::Timeout {
        op: "fokus".into(),
        ms: 1,
    });
    assert_eq!(FocusedField::from_lookup(&err), FocusedField::Unknown);
    assert!(FocusedField::Ordinary.check_typing().is_ok());
    for f in [FocusedField::Password, FocusedField::Unknown] {
        assert!(matches!(f.check_typing(), Err(GuiError::Policy(_))));
    }
}

#[test]
fn editing_batches_are_recognised() {
    let batches = |steps: Vec<InputStep>| {
        plan_batches(
            &InputPlan {
                window: WindowId(1),
                steps,
            },
            &InputPacing::default(),
        )
        .unwrap()
    };
    let text = batches(vec![InputStep::Text { text: "ab".into() }]);
    assert!(batch_writes_text(&text[0].events));
    for k in [
        "Ctrl+V",
        "Shift+Insert",
        "a",
        "5",
        "Space",
        "Backspace",
        "Ctrl+Delete",
    ] {
        let chord = KeyChord::parse(k).unwrap();
        assert!(chord.key.edits_field(), "{k}");
        let b = batches(vec![InputStep::Keys { chord }]);
        assert!(batch_writes_text(&b[0].events), "{k}");
    }
    for k in [
        "Enter",
        "Tab",
        "Shift+Tab",
        "Esc",
        "Left",
        "Ctrl+End",
        "Alt+F4",
        "F5",
    ] {
        let chord = KeyChord::parse(k).unwrap();
        assert!(!chord.key.edits_field(), "{k}");
        let b = batches(vec![InputStep::Keys { chord }]);
        assert!(!batch_writes_text(&b[0].events), "{k}");
    }
    let enter = batches(vec![InputStep::Text { text: "\n".into() }]);
    assert!(!batch_writes_text(&enter[0].events));
    let click = batches(vec![InputStep::Click {
        x: 1,
        y: 1,
        button: MouseButton::Left,
        double: false,
    }]);
    assert!(!batch_writes_text(&click[0].events));
}
