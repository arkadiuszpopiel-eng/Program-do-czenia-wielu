//! Przegląd bezpieczeństwa #2, P2-03: `input_type_text` i skróty wpisujące treść nigdy nie trafiają
//! do pola hasła; fokus nieznany (port bez odczytu fokusu, UIA zawieszone) = odmowa wpisywania;
//! fokus przechodzący do pola hasła w trakcie pisania zatrzymuje kolejne paczki.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use platform_contract::{
    ElementRef, GuiError, ScreenRect, TreeOptions, UiaAction, UiaNode, UiaPort, UiaQuery, UiaText,
    UiaTree, WindowId,
};
use platform_fake::{FakeDesktop, FakeElement, FakeWindow, GuiRecordKind, ScriptEvent};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{DenialReason, Tool, ToolCtx, ToolStatus, Toolset};
use tools_input_contract::InputToolsConfig;
use tools_input_impl::{InputTools, InputToolsDeps};
use watchdog_contract::ManualClock;

fn broker() -> Arc<FakeBroker> {
    let env = PathEnv::windows_profile(r"C:\Users\ala");
    let policy = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    let b = Arc::new(FakeBroker::with(policy, env, Arc::new(ManualClock::new(1_000_000))).unwrap());
    for t in ["tools-input.type_text", "tools-input.keys"] {
        b.script(t, ScriptedDecision::Allow);
    }
    b
}

fn tools(desktop: &Arc<FakeDesktop>, uia: Arc<dyn UiaPort>) -> InputTools {
    InputTools::new(InputToolsDeps {
        desktop: desktop.clone(),
        uia,
        input: desktop.clone(),
        broker: broker(),
        config: InputToolsConfig::default(),
        bus: None,
    })
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(300);
    c
}

fn tool(t: &InputTools, name: &str) -> Arc<dyn Tool> {
    t.tools()
        .into_iter()
        .find(|x| x.manifest().name == name)
        .unwrap()
}

/// Okno logowania: pole loginu i pole hasła (fokus w haśle, jeśli `password_focused`).
fn login_window(password_focused: bool) -> (Arc<FakeDesktop>, WindowId, ElementRef, ElementRef) {
    let d = Arc::new(FakeDesktop::new());
    let rect = ScreenRect::from_xywh(0, 0, 800, 600);
    let w = d.add_window(FakeWindow::new("Bank — logowanie", "bank.exe", rect), true);
    let mut login = FakeElement::new("Login", "edit", ScreenRect::from_xywh(10, 10, 200, 20));
    let mut pass =
        FakeElement::new("Hasło", "edit", ScreenRect::from_xywh(10, 40, 200, 20)).password();
    if password_focused {
        pass = pass.focused();
    } else {
        login = login.focused();
    }
    let login = d.add_element(w, login).unwrap();
    let pass = d.add_element(w, pass).unwrap();
    (d, w, login, pass)
}

fn denied() -> ToolStatus {
    ToolStatus::Denied {
        reason: DenialReason::Policy,
    }
}

fn inputs(d: &FakeDesktop) -> usize {
    d.records()
        .iter()
        .filter(|r| matches!(r.kind, GuiRecordKind::Input(_)))
        .count()
}

#[tokio::test]
async fn type_text_never_reaches_a_focused_password_field() {
    let (d, w, _, _) = login_window(true);
    let t = tools(&d, d.clone());
    let out = tool(&t, "input_type_text")
        .call(json!({"window": w.0, "text": "hunter2"}), &ctx())
        .await;
    assert_eq!(out.status, denied(), "{out:?}");
    assert!(out.text.contains("hasł"), "{}", out.text);
    assert!(d.typed_text(w).is_empty(), "nic nie wpisano");
    assert_eq!(inputs(&d), 0);
}

#[tokio::test]
async fn editing_keys_are_refused_in_password_field_but_enter_submits() {
    let (d, w, _, _) = login_window(true);
    let t = tools(&d, d.clone());
    for keys in [
        json!(["Ctrl+V"]),
        json!(["a", "b"]),
        json!(["Shift+Insert"]),
    ] {
        let out = tool(&t, "input_keys")
            .call(json!({"window": w.0, "keys": keys}), &ctx())
            .await;
        assert_eq!(out.status, denied(), "{keys} {out:?}");
    }
    assert_eq!(inputs(&d), 0);
    let out = tool(&t, "input_keys")
        .call(json!({"window": w.0, "keys": ["Enter"]}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
}

#[tokio::test]
async fn ordinary_field_still_gets_text() {
    let (d, w, _, _) = login_window(false);
    let t = tools(&d, d.clone());
    let out = tool(&t, "input_type_text")
        .call(json!({"window": w.0, "text": "ala@example.com"}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(d.typed_text(w), "ala@example.com");
}

#[tokio::test]
async fn focus_moving_to_password_mid_typing_stops_the_next_batch() {
    let (d, w, _, _) = login_window(false);
    d.script_after(d.injected_batches() + 1, ScriptEvent::FocusPassword(w));
    let t = tools(&d, d.clone());
    let out = tool(&t, "input_type_text")
        .call(json!({"window": w.0, "text": "x".repeat(100)}), &ctx())
        .await;
    assert_eq!(out.status, denied(), "{out:?}");
    assert_eq!(d.typed_text(w).len(), 16, "tylko pierwsza paczka");
}

/// UIA bez odczytu fokusu (domyślna metoda kontraktu) — fokus nieznany.
struct NoFocus(Arc<FakeDesktop>);

impl UiaPort for NoFocus {
    fn tree(&self, window: WindowId, options: &TreeOptions) -> Result<UiaTree, GuiError> {
        self.0.tree(window, options)
    }
    fn find(&self, window: WindowId, query: &UiaQuery) -> Result<Vec<UiaNode>, GuiError> {
        self.0.find(window, query)
    }
    fn element(&self, element: &ElementRef) -> Result<UiaNode, GuiError> {
        self.0.element(element)
    }
    fn read_text(&self, element: &ElementRef, max: usize) -> Result<UiaText, GuiError> {
        self.0.read_text(element, max)
    }
    fn act(&self, element: &ElementRef, action: &UiaAction) -> Result<UiaNode, GuiError> {
        self.0.act(element, action)
    }
    fn password_rects(&self, window: WindowId) -> Result<Vec<ScreenRect>, GuiError> {
        self.0.password_rects(window)
    }
}

#[tokio::test]
async fn unknown_focus_refuses_typing() {
    let (d, w, _, _) = login_window(false);
    let t = tools(&d, Arc::new(NoFocus(d.clone())));
    let out = tool(&t, "input_type_text")
        .call(json!({"window": w.0, "text": "abc"}), &ctx())
        .await;
    assert_eq!(out.status, denied(), "{out:?}");
    assert!(d.typed_text(w).is_empty());
    d.set_uia_hang(true);
    let t = tools(&d, d.clone());
    let out = tool(&t, "input_type_text")
        .call(json!({"window": w.0, "text": "abc"}), &ctx())
        .await;
    assert_ne!(out.status, ToolStatus::Ok, "{out:?}");
    assert!(d.typed_text(w).is_empty());
}

/// Przegląd #3, SR3-04: globalne skróty Alfy rejestruje powłoka Tauri (`RegisterHotKey` bez
/// filtra pochodzenia z P2-04), więc paczka `SendInput` agentki do **dowolnego** okna uruchamia
/// je w Alfie: `Ctrl+Alt+D` — dyktowanie z mikrofonu do okna na pierwszym planie (agentka czyta
/// potem transkrypcję rozmowy w pokoju), `Ctrl+Alt+R` — czytanie na głos, `Ctrl+Alt+Space` —
/// okno Alfy na wierzch. Skrót globalny i tak nie dociera do aplikacji docelowej, więc odmowa
/// niczego nie zabiera.
#[tokio::test]
async fn alfa_global_shortcuts_are_never_sent_by_agents() {
    let d = Arc::new(FakeDesktop::new());
    let rect = ScreenRect::from_xywh(0, 0, 800, 600);
    let w = d.add_window(FakeWindow::new("Notatki", "notepad.exe", rect), true);
    let edit = FakeElement::new("Tekst", "edit", rect).focused();
    d.add_element(w, edit).unwrap();
    let t = tools(&d, d.clone());
    for chord in ["Ctrl+Alt+D", "ctrl+alt+r", "Ctrl+Alt+Space", "Alt+Ctrl+d"] {
        let out = tool(&t, "input_keys")
            .call(json!({"window": w.0, "keys": [chord]}), &ctx())
            .await;
        assert_eq!(out.status, denied(), "{chord}: {out:?}");
    }
    assert_eq!(inputs(&d), 0, "żadne naciśnięcie nie wyszło do systemu");
    // Inne skróty z Ctrl+Alt (np. AltGr na układzie polskim) nadal działają w aplikacji.
    let out = tool(&t, "input_keys")
        .call(json!({"window": w.0, "keys": ["Ctrl+Alt+Shift+D"]}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
}
