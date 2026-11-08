//! Przegląd bezpieczeństwa #2, P2-01 (strażnik celów: wyskakujące okna WebView2 Alfy, okna-
//! własności, ramki UWP, drzewo procesów Alfy liczone przy każdej akcji) i P2-02 (okno chronione,
//! które pojawi się między wyliczeniem okien a zrzutem, jest zamaskowane): 0/500 skutków w oknach
//! Alfy w losowych scenariuszach, także dla procesów WebView2 utworzonych po starcie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use platform_contract::{
    CaptureRequest, CaptureTarget, DesktopPort, GuiError, InputControl, InputPlan, InputPort,
    InputStep, KeyChord, MASK_COLOR, MaskReason, MouseButton, ScreenCapturePort, ScreenRect,
    TargetGuard, TreeOptions, UiaAction, UiaPattern, UiaPort, WindowId, WindowState,
};
use platform_fake::{FakeDesktop, FakeElement, FakeWindow, ScriptEvent};
use proptest::prelude::*;

const WEBVIEW: &str =
    r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application\msedgewebview2.exe";

/// Pulpit: okna zwykłe i okna „Alfy” w różnych przebraniach. Zwraca (pulpit, okna, okna Alfy).
fn scene(xs: &[i32]) -> (FakeDesktop, Vec<WindowId>, HashSet<WindowId>) {
    let d = FakeDesktop::new();
    let me = std::process::id();
    let at =
        |i: usize, w: i32, h: i32| ScreenRect::from_xywh(xs[i % xs.len()], 40 * i as i32, w, h);
    let mut ids = Vec::new();
    let mut alfa = HashSet::new();
    let mut add = |d: &FakeDesktop, spec: FakeWindow, protected: bool, ids: &mut Vec<WindowId>| {
        let id = d.add_window(spec, true);
        d.add_element(
            id,
            FakeElement::new("OK", "button", ScreenRect::from_xywh(0, 0, 1, 1))
                .patterns(&[UiaPattern::Invoke, UiaPattern::Value]),
        );
        ids.push(id);
        if protected {
            alfa.insert(id);
        }
        id
    };
    let notepad = add(
        &d,
        FakeWindow::new("Notatnik", "notepad.exe", at(0, 600, 400)),
        false,
        &mut ids,
    );
    let main = add(
        &d,
        FakeWindow::new("Alfa", "alfa-desktop.exe", at(1, 900, 700)).child_of(1),
        true,
        &mut ids,
    );
    // Proces przeglądarki WebView2 — dziecko Alfy, obraz poza katalogami Alfy.
    let browser = d.add_process(WEBVIEW, Some(me));
    let popup = FakeWindow::new("", WEBVIEW, at(2, 200, 300))
        .in_process(browser)
        .owned_by(main);
    add(&d, popup, true, &mut ids);
    // Menu kontekstowe WebView2 bez właściciela — tylko drzewo procesów.
    add(
        &d,
        FakeWindow::new("", WEBVIEW, at(3, 150, 200)).in_process(browser),
        true,
        &mut ids,
    );
    // Dialog innego procesu będący własnością okna Alfy.
    add(
        &d,
        FakeWindow::new("Zapisz jako", "explorer.exe", at(4, 500, 400)).owned_by(main),
        true,
        &mut ids,
    );
    // Ramki UWP: z rozpoznaną aplikacją (zwykła) i bez (zawieszona — nieznana).
    add(
        &d,
        FakeWindow::uwp("Kalkulator", r"C:\Apps\Calculator.exe", at(5, 300, 400)),
        false,
        &mut ids,
    );
    add(
        &d,
        FakeWindow::uwp("Ustawienia", "", at(6, 600, 500)),
        true,
        &mut ids,
    );
    // Aplikacja UWP Alfy-podobna: treść w procesie Brokera-UI (obraz chroniony).
    add(
        &d,
        FakeWindow::uwp("Zatwierdź", "alfa-broker-ui.exe", at(7, 300, 200)),
        true,
        &mut ids,
    );
    d.focus(notepad).unwrap();
    (d, ids, alfa)
}

#[derive(Debug, Clone)]
enum Op {
    Text(usize, String),
    Keys(usize),
    Click(i32, i32, usize),
    Focus(usize),
    Bounds(usize, i32, i32),
    State(usize, u8),
    Uia(usize),
    RaiseAfter(u64, usize),
    /// Renderer WebView2 odtworzony po awarii: nowy proces-dziecko przeglądarki i jego okno.
    Respawn(i32, i32),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..12usize, "[a-zżół ]{1,40}").prop_map(|(w, t)| Op::Text(w, t)),
        (0..12usize).prop_map(Op::Keys),
        (0..1920i32, 0..1080i32, 0..12usize).prop_map(|(x, y, w)| Op::Click(x, y, w)),
        (0..12usize).prop_map(Op::Focus),
        (0..12usize, 0..1500i32, 0..900i32).prop_map(|(w, x, y)| Op::Bounds(w, x, y)),
        (0..12usize, 0..3u8).prop_map(|(w, s)| Op::State(w, s)),
        (0..12usize).prop_map(Op::Uia),
        (1..5u64, 0..12usize).prop_map(|(n, w)| Op::RaiseAfter(n, w)),
        (0..1500i32, 0..800i32).prop_map(|(x, y)| Op::Respawn(x, y)),
    ]
}

fn apply(d: &FakeDesktop, ids: &mut Vec<WindowId>, alfa: &mut HashSet<WindowId>, op: &Op) {
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
                chord: KeyChord::parse("Ctrl+V").unwrap(),
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
        Op::Uia(w) => {
            if let Ok(tree) = d.tree(pick(w), &TreeOptions::default()) {
                for n in &tree.nodes {
                    let _ = d.act(&n.element, &UiaAction::Invoke);
                }
            }
        }
        Op::RaiseAfter(n, w) => {
            d.script_after(d.injected_batches() + n, ScriptEvent::Raise(pick(w)));
        }
        Op::Respawn(x, y) => {
            let browser = d.add_process(WEBVIEW, Some(std::process::id()));
            let renderer = d.add_process(WEBVIEW, Some(browser));
            let id = d.add_window(
                FakeWindow::new("", WEBVIEW, ScreenRect::from_xywh(*x, *y, 200, 150))
                    .in_process(renderer),
                false,
            );
            ids.push(id);
            alfa.insert(id);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn zero_effects_in_alfa_popups_uwp_and_respawned_webview(
        xs in prop::collection::vec(0..1300i32, 1..8),
        ops in prop::collection::vec(op(), 1..14),
    ) {
        let (d, mut ids, mut alfa) = scene(&xs);
        for o in &ops {
            apply(&d, &mut ids, &mut alfa, o);
        }
        let bad: Vec<_> = d.records().into_iter().filter(|r| alfa.contains(&r.window)).collect();
        prop_assert!(bad.is_empty(), "skutki w oknach Alfy: {bad:?}");
        // Okna Alfy są oznaczone jako chronione (maskowanie zrzutów, lista okien).
        let listed = d.windows().unwrap();
        for w in listed.iter().filter(|w| alfa.contains(&w.id)) {
            prop_assert!(w.protected, "{w:?}");
        }
    }
}

#[test]
fn old_check_by_window_process_alone_missed_these_windows() {
    let (d, _, alfa) = scene(&[0]);
    let guard = TargetGuard::baseline();
    let listed = d.windows().unwrap();
    // Reprodukcja luki: strażnik liczący tylko proces okna (stan przed poprawką) nie chronił
    // wyskakujących okien WebView2, dialogów-własności ani ramek UWP.
    let missed: Vec<_> = listed
        .iter()
        .filter(|w| alfa.contains(&w.id) && !guard.is_protected(w.pid, &w.image))
        .collect();
    assert!(missed.len() >= 3, "{missed:?}");
    assert!(
        missed.iter().all(|w| w.protected),
        "teraz chronione: {missed:?}"
    );
    // Okno UWP z rozpoznaną aplikacją nosi obraz aplikacji (deny-lista, `gui.control(app)`).
    let calc = listed.iter().find(|w| w.title == "Kalkulator").unwrap();
    assert!(calc.image.ends_with("Calculator.exe") && !calc.protected);
    assert!(d.focus(calc.id).is_ok());
}

#[test]
fn protected_window_appearing_during_capture_is_masked() {
    let d = FakeDesktop::new();
    d.add_window(
        FakeWindow::new(
            "Notatnik",
            "notepad.exe",
            ScreenRect::from_xywh(0, 0, 1920, 1080),
        ),
        true,
    );
    // Broker-UI wyskakuje między wyliczeniem okien a przechwyceniem klatki.
    d.show_during_capture(FakeWindow::new(
        "Zatwierdzenie",
        "alfa-broker-ui.exe",
        ScreenRect::from_xywh(100, 100, 400, 300),
    ));
    let mut req = CaptureRequest::new(CaptureTarget::Monitor { index: 0 });
    req.max_width = 1920;
    req.max_height = 1080;
    let shot = d.capture(&req).unwrap();
    let img = d.last_capture().unwrap();
    assert_eq!(
        img.pixel(300, 250),
        Some(MASK_COLOR),
        "Broker-UI zamaskowany"
    );
    assert!(
        shot.masked
            .iter()
            .any(|m| m.reason == MaskReason::ProtectedWindow)
    );
    // Zbiór okien zmienia się przy każdej próbie: po wyczerpaniu prób maska obejmuje sumę obu
    // wyliczeń i okna zmienione w całości (fail-closed).
    for i in 0..5 {
        d.show_during_capture(FakeWindow::new(
            "Hasła",
            "keepass.exe",
            ScreenRect::from_xywh(600 + i * 10, 500, 300, 200),
        ));
    }
    let _ = d.capture(&req).unwrap();
    let img = d.last_capture().unwrap();
    assert_eq!(
        img.pixel(700, 600),
        Some(MASK_COLOR),
        "menedżer haseł zamaskowany"
    );
    assert!(matches!(
        d.capture(&CaptureRequest::new(CaptureTarget::Window {
            window: WindowId(999)
        })),
        Err(GuiError::ElementNotFound(_))
    ));
}
