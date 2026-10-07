//! Usługa na wirtualnym pulpicie (`platform-fake`): kontrakt, porcje atomowe przy przerwaniu,
//! pole hasła z fokusem (UIA), property: 0 wpisów do okien chronionych i do okien innych niż cel.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use common::Desk;
use platform_contract::{
    DesktopPort, ElementRef, GuiError, ScreenRect, TargetGuard, TreeOptions, UiaAction, UiaNode,
    UiaPort, UiaQuery, UiaText, UiaTree, WindowId,
};
use platform_fake::{FakeElement, FakeWindow, GuiRecordKind, ScriptEvent};
use proptest::prelude::*;
use voice_dictation_contract::contract_tests::{self, DesktopDriver};
use voice_dictation_contract::{
    Dictation, DictationCfg, DictationError, DictationMode, DictationPhase, PauseReason,
    RefuseReason,
};
use voice_dictation_impl::{DictationPorts, DictationService, chunks};

#[test]
fn contract_suite() {
    contract_tests::run_all(|| {
        let desk = Desk::new();
        (desk.service(), Box::new(desk) as Box<dyn DesktopDriver>)
    });
}

#[test]
fn chunks_are_atomic_batches() {
    assert_eq!(chunks("abcdefghij"), vec!["abcdefgh", "ij"]);
    assert!(
        chunks("😀😀😀😀😀")
            .iter()
            .all(|c| c.encode_utf16().count() <= 8)
    );
    assert!(chunks("").is_empty());
}

#[test]
fn interruption_keeps_exact_remainder() {
    let desk = Desk::new();
    let editor = WindowId(desk.open_app("notepad.exe"));
    let popup = desk.0.add_window(
        FakeWindow::new(
            "Okno",
            r"C:\Apps\x.exe",
            ScreenRect::from_xywh(0, 0, 50, 50),
        ),
        false,
    );
    // Po 2 porcjach na wierzch wyskakuje inne okno (przejmuje fokus).
    desk.0
        .script_after(desk.0.injected_batches() + 2, ScriptEvent::Raise(popup));
    let mut d = desk.service();
    d.start(DictationMode::Toggle, 0).unwrap();
    d.on_final("to jest dłuższe zdanie do wpisania", 10)
        .unwrap();
    assert_eq!(
        d.status().phase,
        DictationPhase::Paused(PauseReason::FocusChanged)
    );
    let typed = desk.replay(editor);
    assert_eq!(typed, "To jest dłuższe ", "dokładnie 2 porcje po 8 znaków");
    assert_eq!(desk.replay(popup), "");
    DesktopPort::focus(desk.0.as_ref(), editor).unwrap();
    d.tick(20);
    assert_eq!(desk.replay(editor), "To jest dłuższe zdanie do wpisania");
    // Wejście użytkownika → pauza, potem wznowienie po 1,5 s.
    desk.0.physical_input();
    d.on_final("dalej", 30).unwrap();
    assert_eq!(
        d.status().phase,
        DictationPhase::Paused(PauseReason::UserTyping)
    );
    desk.0.advance(2_000);
    d.tick(2_000);
    assert_eq!(
        desk.replay(editor),
        "To jest dłuższe zdanie do wpisania dalej"
    );
}

/// UIA z fokusem na wskazanym elemencie (atrapa pulpitu nie modeluje fokusu elementów).
struct FocusOverlay {
    inner: Arc<platform_fake::FakeDesktop>,
    focused: Mutex<Option<ElementRef>>,
}

impl UiaPort for FocusOverlay {
    fn tree(&self, window: WindowId, options: &TreeOptions) -> Result<UiaTree, GuiError> {
        let mut t = self.inner.tree(window, options)?;
        let f = self.focused.lock().unwrap().clone();
        for n in &mut t.nodes {
            n.focused = Some(&n.element) == f.as_ref();
        }
        Ok(t)
    }
    fn find(&self, window: WindowId, query: &UiaQuery) -> Result<Vec<UiaNode>, GuiError> {
        self.inner.find(window, query)
    }
    fn element(&self, element: &ElementRef) -> Result<UiaNode, GuiError> {
        self.inner.element(element)
    }
    fn read_text(&self, element: &ElementRef, max: usize) -> Result<UiaText, GuiError> {
        self.inner.read_text(element, max)
    }
    fn act(&self, element: &ElementRef, action: &UiaAction) -> Result<UiaNode, GuiError> {
        self.inner.act(element, action)
    }
    fn password_rects(&self, window: WindowId) -> Result<Vec<ScreenRect>, GuiError> {
        self.inner.password_rects(window)
    }
}

#[test]
fn password_field_with_focus_is_refused_and_login_field_allowed() {
    let desk = Desk::new();
    let rect = ScreenRect::from_xywh(100, 100, 400, 300);
    let w = desk.0.add_window(
        FakeWindow::new("Logowanie", r"C:\Apps\bank.exe", rect),
        true,
    );
    let login = desk
        .0
        .add_element(w, FakeElement::new("Login", "edit", rect))
        .unwrap();
    let pass = desk
        .0
        .add_element(w, FakeElement::new("Hasło", "edit", rect).password())
        .unwrap();
    let overlay = Arc::new(FocusOverlay {
        inner: desk.0.clone(),
        focused: Mutex::new(Some(login)),
    });
    let ports = DictationPorts {
        desktop: desk.0.clone(),
        uia: overlay.clone(),
        input: desk.0.clone(),
    };
    let mut d = DictationService::new(ports, DictationCfg::default());
    d.start(DictationMode::Toggle, 0).unwrap();
    d.on_final("jan kowalski", 10).unwrap();
    assert_eq!(desk.replay(w), "Jan kowalski");
    // Fokus przechodzi do pola hasła (Tab) — następna fraza nie jest wpisywana, sesja kończy się.
    *overlay.focused.lock().unwrap() = Some(pass);
    d.on_final("tajne hasło", 20).unwrap();
    assert_eq!(desk.replay(w), "Jan kowalski");
    assert_eq!(d.status().phase, DictationPhase::Idle);
    assert_eq!(
        d.start(DictationMode::Toggle, 30).err(),
        Some(DictationError::Refused(RefuseReason::PasswordField))
    );
    desk.0.set_uia_hang(true);
    *overlay.focused.lock().unwrap() = None;
    assert!(
        d.start(DictationMode::Toggle, 40).is_err(),
        "UIA wisi — fail-closed"
    );
}

#[derive(Debug, Clone)]
enum Op {
    Final(u8),
    FocusOther(u8),
    FocusTarget,
    Undo,
    Tick,
    RaiseProtected(u8),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u8..6).prop_map(Op::Final),
        (0u8..3).prop_map(Op::FocusOther),
        Just(Op::FocusTarget),
        Just(Op::Undo),
        Just(Op::Tick),
        (0u8..3).prop_map(Op::RaiseProtected),
    ]
}

const PHRASES: [&str; 6] = [
    "ala ma kota kropka",
    "nowa linia dalej",
    "cofnij to",
    "dwadzieścia trzy przecinek",
    "znak zapytania",
    "koniec dyktowania",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// 0 wpisów do okien Alfy/Brokera i do okien innych niż cel z chwili startu.
    #[test]
    fn never_types_outside_start_target(ops in prop::collection::vec(op(), 1..40)) {
        let desk = Desk::new();
        let rect = ScreenRect::from_xywh(0, 0, 800, 600);
        let protected: Vec<WindowId> = ["alfa-desktop.exe", "alfa-broker-ui.exe", "alfa.exe"]
            .iter()
            .map(|i| desk.0.add_window(FakeWindow::new(i, i, rect), false))
            .collect();
        let others: Vec<WindowId> = ["chrome.exe", "code.exe", "winword.exe"]
            .iter()
            .map(|i| desk.0.add_window(FakeWindow::new(i, i, rect), false))
            .collect();
        let target = WindowId(desk.open_app("notepad.exe"));
        let mut d = desk.service();
        d.start(DictationMode::Toggle, 0).unwrap();
        let mut now = 0;
        for o in ops {
            now += 100;
            match o {
                Op::Final(i) => { let _ = d.on_final(PHRASES[usize::from(i)], now); }
                Op::FocusOther(i) => DesktopPort::focus(desk.0.as_ref(), others[usize::from(i)]).unwrap(),
                Op::FocusTarget => DesktopPort::focus(desk.0.as_ref(), target).unwrap(),
                Op::Undo => { let _ = d.undo_last(now); }
                Op::Tick => { d.tick(now); }
                Op::RaiseProtected(i) => {
                    desk.0.script_after(desk.0.injected_batches() + 1, ScriptEvent::Raise(protected[usize::from(i)]));
                }
            }
        }
        let guard = TargetGuard::baseline();
        let inputs = desk.0.records().into_iter().filter(|r| !matches!(r.kind, GuiRecordKind::Window(_)));
        for r in inputs {
            prop_assert!(!guard.is_protected(r.pid, &r.image), "wpis do okna chronionego: {r:?}");
            prop_assert_eq!(r.window, target, "wpis poza celem: {:?}", r);
        }
    }
}
