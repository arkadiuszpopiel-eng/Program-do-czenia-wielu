//! Fala 5 — logika kontraktu `platform-contract` sprawdzana na atrapie (tu, a nie w `src/`, bo
//! crate kontraktu jest na granicy limitu 8 000 linii):
//! - F6-03: próg „ubogiego” drzewa UIA liczy tylko obszar klienta (bez ramy okna i paska tytułu);
//! - PT-25: menedżery haseł i okno poświadczeń Windows są celami chronionymi także dla UIA,
//!   wejścia i operacji na oknach (dotąd maskowane tylko w zrzutach).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use platform_contract::{
    CaptureRequest, CaptureTarget, DEFAULT_MASKED_APPS, DesktopPort, ElementRef, GuiError,
    InputControl, InputPlan, InputPort, InputStep, MaskReason, SENSITIVE_APPS, SPARSE_TREE_NODES,
    ScreenCapturePort, ScreenRect, TargetGuard, TreeOptions, UiaAction, UiaNode, UiaPattern,
    UiaPort, UiaQuery, UiaTree, WindowId,
};
use platform_fake::{FakeDesktop, FakeElement, FakeWindow};

fn node(depth: u16, role: &str, automation_id: &str) -> UiaNode {
    UiaNode {
        element: ElementRef::parse("w1:42.1").unwrap(),
        depth,
        pid: 7,
        role: role.into(),
        name: "x".into(),
        automation_id: automation_id.into(),
        class_name: String::new(),
        value: None,
        is_password: false,
        enabled: true,
        offscreen: false,
        focused: false,
        toggle: None,
        expand: None,
        selected: None,
        rect: ScreenRect::from_xywh(0, 0, 10, 10),
        patterns: vec![UiaPattern::Invoke],
    }
}

/// Rama okna Win32 w widoku kontrolek UIA: okno, pasek tytułu z menu systemowym i przyciskami.
fn frame() -> Vec<UiaNode> {
    vec![
        node(0, "window", ""),
        node(1, "title_bar", "TitleBar"),
        node(2, "menu_bar", "SystemMenuBar"),
        node(3, "menu_item", "Item 1"),
        node(2, "button", "Minimize"),
        node(2, "button", "Maximize"),
        node(2, "button", "Close"),
    ]
}

fn tree(nodes: Vec<UiaNode>) -> UiaTree {
    UiaTree {
        window: WindowId(1),
        nodes,
        truncated: false,
    }
}

#[test]
fn frame_only_tree_is_sparse() {
    // Okno, którego klient to jedna płaszczyzna (płótno, GTK, Java bez Access Bridge, sesja
    // zdalna): 7 węzłów ramy + 1 węzeł klienta. Dotąd 8 > 5 → `sparse = false`.
    let mut nodes = frame();
    nodes.push(node(1, "pane", "Canvas"));
    let t = tree(nodes);
    assert_eq!(t.client_nodes(), 1);
    assert!(t.is_sparse(), "sama rama + płótno = drzewo ubogie");
    let bare = tree(frame());
    assert_eq!(bare.client_nodes(), 0);
    assert!(bare.is_sparse());
}

#[test]
fn client_nodes_after_the_title_bar_still_count() {
    // Po poddrzewie paska tytułu (powrót do głębokości 1) węzły klienta liczą się dalej; systemowy
    // pasek menu poza paskiem tytułu też jest ramą.
    let mut nodes = frame();
    nodes.push(node(1, "menu_bar", "SystemMenuBar"));
    nodes.push(node(2, "menu_item", ""));
    nodes.extend([
        node(1, "menu_bar", "MenuBar"),
        node(2, "menu_item", "Plik"),
        node(2, "menu_item", "Edycja"),
        node(1, "document", "Edytor"),
        node(1, "status_bar", ""),
    ]);
    let t = tree(nodes);
    assert_eq!(t.client_nodes(), SPARSE_TREE_NODES);
    assert!(t.is_sparse(), "5 węzłów klienta = próg");
    let mut more = t.clone();
    more.nodes.push(node(2, "text", "Wiersz 1"));
    assert_eq!(more.client_nodes(), SPARSE_TREE_NODES + 1);
    assert!(!more.is_sparse());
}

#[test]
fn fake_window_with_frame_only_reports_sparse_tree() {
    let d = FakeDesktop::new();
    let r = ScreenRect::from_xywh(0, 0, 100, 30);
    let w = d.add_window(
        FakeWindow::new(
            "Płótno",
            r"C:\Apps\paint.exe",
            ScreenRect::from_xywh(0, 0, 800, 600),
        ),
        true,
    );
    for n in frame() {
        d.add_element(
            w,
            FakeElement::new(&n.name, &n.role, r)
                .depth(n.depth)
                .automation_id(&n.automation_id),
        );
    }
    d.add_element(w, FakeElement::new("", "pane", r).automation_id("Canvas"));
    let t = d.tree(w, &TreeOptions::default()).unwrap();
    assert_eq!(t.nodes.len(), 8);
    assert!(t.is_sparse());
}

const MANAGERS: [&str; 6] = [
    r"C:\Program Files\KeePass Password Safe 2\KeePass.exe",
    r"C:\Program Files\KeePassXC\KeePassXC.exe",
    r"C:\Users\ala\AppData\Local\1Password\app\8\1Password.exe",
    r"C:\Users\ala\AppData\Local\Programs\Bitwarden\Bitwarden.exe",
    r"C:\Windows\System32\CredentialUIBroker.exe",
    "ENPASS~1.EXE",
];

#[test]
fn sensitive_apps_cover_the_capture_deny_list() {
    for app in DEFAULT_MASKED_APPS {
        assert!(SENSITIVE_APPS.contains(&app), "{app}");
    }
    let g = TargetGuard::baseline();
    for image in MANAGERS {
        assert!(g.is_protected(4321, image), "{image}");
        let err = g.check(4321, image, "klik").unwrap_err();
        assert!(
            matches!(&err, GuiError::ProtectedTarget(m) if m.contains("menedżer haseł")),
            "{err:?}"
        );
    }
    assert!(!g.is_protected(4321, r"C:\Windows\notepad.exe"));
    assert!(!g.is_protected(4321, r"C:\Apps\keepass-notes.exe"));
}

#[test]
fn password_managers_are_protected_targets_for_uia_input_and_windows() {
    let d = FakeDesktop::new();
    let r = ScreenRect::from_xywh(10, 10, 50, 20);
    for (i, image) in MANAGERS.iter().enumerate() {
        let w = d.add_window(
            FakeWindow::new(
                "Baza haseł",
                image,
                ScreenRect::from_xywh(100 * i32::try_from(i).unwrap(), 0, 300, 200),
            ),
            true,
        );
        let e = d
            .add_element(
                w,
                FakeElement::new("bank.example — ala", "list_item", r)
                    .patterns(&[UiaPattern::Invoke, UiaPattern::Value])
                    .value("ala@example.com")
                    .text("Notatka wpisu: PIN 4321"),
            )
            .unwrap();
        let info = d.window(w).unwrap();
        assert!(info.protected, "{image}: okno menedżera haseł chronione");
        let refused = |r: Result<(), GuiError>, what: &str| {
            assert!(
                matches!(r, Err(GuiError::ProtectedTarget(_))),
                "{image} {what}: {r:?}"
            );
        };
        refused(d.tree(w, &TreeOptions::default()).map(|_| ()), "drzewo");
        let q = UiaQuery {
            name_contains: Some("bank".into()),
            max_results: 5,
            ..UiaQuery::default()
        };
        refused(d.find(w, &q).map(|_| ()), "wyszukiwanie");
        refused(d.element(&e).map(|_| ()), "element");
        refused(d.read_text(&e, 100).map(|_| ()), "tekst");
        refused(d.act(&e, &UiaAction::Invoke).map(|_| ()), "akcja");
        let plan = InputPlan {
            window: w,
            steps: vec![InputStep::Text { text: "x".into() }],
        };
        refused(d.send(&plan, &InputControl::new()).map(|_| ()), "wejście");
        refused(d.focus(w), "fokus");
        refused(
            d.set_bounds(w, ScreenRect::from_xywh(0, 0, 400, 300)),
            "przesunięcie",
        );
    }
    assert!(d.records().is_empty(), "zero skutków: {:?}", d.records());
}

#[test]
fn password_manager_stays_masked_as_masked_app_on_screenshots() {
    let d = FakeDesktop::new();
    d.add_window(
        FakeWindow::new(
            "Baza haseł",
            MANAGERS[0],
            ScreenRect::from_xywh(100, 100, 300, 200),
        ),
        true,
    );
    let shot = d
        .capture(&CaptureRequest::new(CaptureTarget::Monitor { index: 0 }))
        .unwrap();
    assert!(
        shot.masked
            .iter()
            .any(|m| m.reason == MaskReason::MaskedApp),
        "{:?}",
        shot.masked
    );
}
