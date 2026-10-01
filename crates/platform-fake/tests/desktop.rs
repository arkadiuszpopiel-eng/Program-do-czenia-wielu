//! Wirtualny pulpit (F6): 0 skutków w oknach Alfy/Brokera w 200 losowych scenariuszach
//! (wejście, kliknięcia w dowolnym punkcie, UIA, zmiany okien, Broker-UI wyskakujący w trakcie),
//! pierwszeństwo fizycznego wejścia, redakcja haseł, maskowanie zrzutów, limit czasu UIA.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use platform_contract::{
    CaptureRequest, CaptureTarget, DesktopPort, GuiError, InputControl, InputPlan, InputPort,
    InputStep, KeyChord, MASK_COLOR, MaskReason, MouseButton, ScreenCapturePort, ScreenRect,
    TargetGuard, TreeOptions, UiaAction, UiaPattern, UiaPort, UiaQuery, WindowId, WindowState,
};
use platform_fake::{FakeDesktop, FakeElement, FakeWindow, PASSWORD_COLOR, ScriptEvent};
use proptest::prelude::*;

const IMAGES: [&str; 8] = [
    "notepad.exe",
    r"C:\Program Files\Microsoft Office\WINWORD.EXE",
    "alfa.exe",
    r"C:\Alfa\alfa-broker-ui.exe",
    "ALFA-DESKTOP.EXE",
    "explorer.exe",
    "",
    "ALFA-B~1.EXE",
];

#[derive(Debug, Clone)]
enum Op {
    Text(usize, String),
    Keys(usize),
    Click(i32, i32, usize),
    Focus(usize),
    Bounds(usize, i32, i32),
    State(usize, u8),
    Uia(usize, usize),
    RaiseAfter(u64, usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..8usize, "[a-zżółć ]{1,60}").prop_map(|(w, t)| Op::Text(w, t)),
        (0..8usize).prop_map(Op::Keys),
        (0..1920i32, 0..1080i32, 0..8usize).prop_map(|(x, y, w)| Op::Click(x, y, w)),
        (0..8usize).prop_map(Op::Focus),
        (0..8usize, 0..1500i32, 0..900i32).prop_map(|(w, x, y)| Op::Bounds(w, x, y)),
        (0..8usize, 0..3u8).prop_map(|(w, s)| Op::State(w, s)),
        (0..8usize, 0..3usize).prop_map(|(w, e)| Op::Uia(w, e)),
        (1..6u64, 0..8usize).prop_map(|(n, w)| Op::RaiseAfter(n, w)),
    ]
}

fn build(spec: &[(usize, i32, i32, bool)]) -> (FakeDesktop, Vec<WindowId>) {
    let d = FakeDesktop::new();
    let mut ids = Vec::new();
    for (i, (img, x, y, focus)) in spec.iter().enumerate() {
        let rect = ScreenRect::from_xywh(*x, *y, 600, 400);
        let id = d.add_window(
            FakeWindow::new(&format!("okno {i}"), IMAGES[*img], rect),
            *focus,
        );
        for e in 0..3 {
            let r = ScreenRect::from_xywh(x + 10 + e * 50, y + 10, 40, 20);
            d.add_element(
                id,
                FakeElement::new(&format!("przycisk {e}"), "button", r)
                    .patterns(&[UiaPattern::Invoke, UiaPattern::Value]),
            );
        }
        ids.push(id);
    }
    (d, ids)
}

fn apply(d: &FakeDesktop, ids: &[WindowId], op: &Op) {
    let pick = |i: &usize| ids[i % ids.len()];
    let send = |w: WindowId, steps: Vec<InputStep>| {
        let _ = d.send(&InputPlan { window: w, steps }, &InputControl::new());
    };
    match op {
        Op::Text(w, t) => {
            let _ = d.focus(pick(w));
            send(pick(w), vec![InputStep::Text { text: t.clone() }]);
        }
        Op::Keys(w) => send(
            pick(w),
            vec![InputStep::Keys {
                chord: KeyChord::parse("Ctrl+S").unwrap(),
            }],
        ),
        Op::Click(x, y, w) => send(
            pick(w),
            vec![InputStep::Click {
                x: *x,
                y: *y,
                button: MouseButton::Left,
                double: false,
            }],
        ),
        Op::Focus(w) => {
            let _ = d.focus(pick(w));
        }
        Op::Bounds(w, x, y) => {
            let _ = d.set_bounds(pick(w), ScreenRect::from_xywh(*x, *y, 300, 200));
        }
        Op::State(w, s) => {
            let state = [
                WindowState::Normal,
                WindowState::Minimized,
                WindowState::Maximized,
            ][*s as usize];
            let _ = d.set_state(pick(w), state);
        }
        Op::Uia(w, e) => {
            if let Ok(tree) = d.tree(pick(w), &TreeOptions::default())
                && let Some(n) = tree.nodes.get(*e)
            {
                let _ = d.act(&n.element, &UiaAction::Invoke);
            }
            // Także odwołanie „zgadnięte” do okna chronionego.
            let guessed = platform_contract::ElementRef {
                window: pick(w),
                runtime_id: vec![42, 1, 0],
            };
            let _ = d.act(&guessed, &UiaAction::SetValue { value: "x".into() });
        }
        Op::RaiseAfter(n, w) => {
            d.script_after(d.injected_batches() + n, ScriptEvent::Raise(pick(w)))
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn no_effect_ever_reaches_protected_windows(
        spec in prop::collection::vec((0..8usize, 0..1300i32, 0..680i32, any::<bool>()), 2..7),
        ops in prop::collection::vec(op(), 1..12),
    ) {
        let (d, ids) = build(&spec);
        for o in &ops {
            apply(&d, &ids, o);
        }
        let guard = TargetGuard::baseline();
        let bad: Vec<_> = d.records().into_iter().filter(|r| guard.is_protected(r.pid, &r.image)).collect();
        prop_assert!(bad.is_empty(), "skutki w oknach chronionych: {bad:?}");
    }
}

fn desktop_with_notepad() -> (FakeDesktop, WindowId) {
    let d = FakeDesktop::new();
    let w = d.add_window(
        FakeWindow::new(
            "Notatnik",
            "notepad.exe",
            ScreenRect::from_xywh(0, 0, 800, 600),
        ),
        true,
    );
    (d, w)
}

#[test]
fn physical_input_interrupts_and_blocks_start() {
    let (d, w) = desktop_with_notepad();
    d.script_after(2, ScriptEvent::PhysicalInput);
    let plan = InputPlan {
        window: w,
        steps: vec![InputStep::Text {
            text: "a".repeat(200),
        }],
    };
    assert_eq!(
        d.send(&plan, &InputControl::new()),
        Err(GuiError::UserInterrupted { sent: 2 })
    );
    assert_eq!(d.typed_text(w).len(), 32, "dwie paczki po 16 znaków");
    assert_eq!(
        d.send(&plan, &InputControl::new()),
        Err(GuiError::UserActive)
    );
    d.advance(5_000);
    let short = InputPlan {
        window: w,
        steps: vec![InputStep::Text {
            text: "Zażółć 😀\n".into(),
        }],
    };
    assert!(d.send(&short, &InputControl::new()).is_ok());
    assert!(d.typed_text(w).ends_with("Zażółć 😀\n"));
}

#[test]
fn broker_ui_raised_mid_typing_gets_nothing() {
    let (d, w) = desktop_with_notepad();
    let broker = d.add_window(
        FakeWindow::new(
            "Alfa — zatwierdzenie",
            "alfa-broker-ui.exe",
            ScreenRect::from_xywh(100, 100, 400, 300),
        ),
        false,
    );
    d.focus(w).unwrap();
    d.script_after(1, ScriptEvent::Raise(broker));
    let plan = InputPlan {
        window: w,
        steps: vec![InputStep::Text {
            text: "x".repeat(64),
        }],
    };
    assert!(matches!(
        d.send(&plan, &InputControl::new()),
        Err(GuiError::ProtectedTarget(_))
    ));
    assert!(d.typed_text(broker).is_empty());
    assert!(d.focus(broker).is_err() && d.set_state(broker, WindowState::Minimized).is_err());
    let click = InputPlan {
        window: w,
        steps: vec![InputStep::Click {
            x: 150,
            y: 150,
            button: MouseButton::Left,
            double: false,
        }],
    };
    assert!(d.send(&click, &InputControl::new()).is_err());
    assert!(d.records().iter().all(|r| r.window != broker));
}

#[test]
fn uia_reads_redact_passwords_and_time_out() {
    let (d, w) = desktop_with_notepad();
    let pw = d
        .add_element(
            w,
            FakeElement::new("Hasło", "edit", ScreenRect::from_xywh(10, 10, 100, 20))
                .patterns(&[UiaPattern::Value])
                .value("tajne")
                .password(),
        )
        .unwrap();
    let doc = d
        .add_element(
            w,
            FakeElement::new(
                "Dokument",
                "document",
                ScreenRect::from_xywh(10, 40, 500, 400),
            )
            .text("treść dokumentu"),
        )
        .unwrap();
    let tree = d.tree(w, &TreeOptions::default()).unwrap();
    assert!(
        tree.nodes
            .iter()
            .all(|n| n.value.as_deref() != Some("tajne"))
    );
    assert_eq!(d.element(&pw).unwrap().value, None);
    assert!(matches!(
        d.act(&pw, &UiaAction::SetValue { value: "x".into() }),
        Err(GuiError::Policy(_))
    ));
    assert!(d.read_text(&pw, 100).is_err());
    assert_eq!(d.read_text(&doc, 5).unwrap().text, "treść");
    let found = d
        .find(
            w,
            &UiaQuery {
                name_contains: Some("dok".into()),
                max_results: 5,
                ..UiaQuery::default()
            },
        )
        .unwrap();
    assert_eq!(found.len(), 1);
    assert!(d.find(w, &UiaQuery::default()).is_err());
    d.set_uia_hang(true);
    assert!(matches!(
        d.tree(w, &TreeOptions::default()),
        Err(GuiError::Timeout { .. })
    ));
}

#[test]
fn capture_masks_protected_windows_passwords_and_unverified() {
    let d = FakeDesktop::new();
    let a = d.add_window(
        FakeWindow::new(
            "Notatnik",
            "notepad.exe",
            ScreenRect::from_xywh(0, 0, 1000, 800),
        ),
        true,
    );
    d.add_element(
        a,
        FakeElement::new("Hasło", "edit", ScreenRect::from_xywh(100, 100, 200, 30)).password(),
    );
    let alfa = d.add_window(
        FakeWindow::new(
            "Alfa",
            "alfa-desktop.exe",
            ScreenRect::from_xywh(1000, 0, 900, 800),
        ),
        false,
    );
    let b = d.add_window(
        FakeWindow::new(
            "Inna",
            "explorer.exe",
            ScreenRect::from_xywh(0, 800, 600, 200),
        ),
        false,
    );
    d.fail_password_check(b);
    let mut req = CaptureRequest::new(CaptureTarget::Monitor { index: 0 });
    req.max_width = 1920;
    req.max_height = 1080;
    let shot = d.capture(&req).unwrap();
    let img = d.last_capture().unwrap();
    assert!(
        img.pixels.chunks_exact(4).all(|p| p != PASSWORD_COLOR),
        "hasło zamaskowane"
    );
    assert_eq!(
        img.pixel(1500, 400),
        Some(MASK_COLOR),
        "okno Alfy zamaskowane"
    );
    assert_eq!(
        img.pixel(300, 900),
        Some(MASK_COLOR),
        "okno niesprawdzone zamaskowane"
    );
    let reasons: Vec<MaskReason> = shot.masked.iter().map(|m| m.reason).collect();
    assert!(
        reasons.contains(&MaskReason::ProtectedWindow)
            && reasons.contains(&MaskReason::PasswordField)
            && reasons.contains(&MaskReason::Unverified)
    );
    assert!(shot.png.starts_with(&[0x89, b'P', b'N', b'G']));
    assert!(matches!(
        d.capture(&CaptureRequest::new(CaptureTarget::Window { window: alfa })),
        Err(GuiError::ProtectedTarget(_))
    ));
    let blocked = d.add_window(
        FakeWindow {
            capture_blocked: true,
            ..FakeWindow::new("DRM", "player.exe", ScreenRect::from_xywh(0, 0, 200, 200))
        },
        false,
    );
    assert!(
        d.capture(&CaptureRequest::new(CaptureTarget::Window {
            window: blocked
        }))
        .unwrap()
        .black_frame
    );
    let mut keepass = FakeWindow::new(
        "KeePass",
        "KeePass.exe",
        ScreenRect::from_xywh(0, 0, 200, 200),
    );
    keepass.color = [1, 2, 3, 255];
    let k = d.add_window(keepass, false);
    assert!(matches!(
        d.capture(&CaptureRequest::new(CaptureTarget::Window { window: k })),
        Err(GuiError::Policy(_))
    ));
}

#[test]
fn window_operations_respect_bounds_and_state() {
    let (d, w) = desktop_with_notepad();
    assert!(
        d.set_bounds(w, ScreenRect::from_xywh(5000, 0, 300, 300))
            .is_err()
    );
    d.set_bounds(w, ScreenRect::from_xywh(10, 10, 300, 300))
        .unwrap();
    d.set_state(w, WindowState::Maximized).unwrap();
    assert_eq!(
        d.window(w).unwrap().rect,
        ScreenRect::from_xywh(0, 0, 1920, 1040)
    );
    d.set_state(w, WindowState::Normal).unwrap();
    assert_eq!(
        d.window(w).unwrap().rect,
        ScreenRect::from_xywh(10, 10, 300, 300)
    );
    d.set_state(w, WindowState::Minimized).unwrap();
    assert!(d.foreground().unwrap().is_none());
    d.focus(w).unwrap();
    assert_eq!(d.foreground().unwrap().unwrap().id, w);
    assert_eq!(d.window_at(20, 20).unwrap(), Some(w));
    assert_eq!(d.monitors().unwrap().len(), 1);
}
