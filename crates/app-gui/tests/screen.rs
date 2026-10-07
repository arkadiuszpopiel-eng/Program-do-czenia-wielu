//! Computer use w aplikacji na wirtualnym pulpicie (`platform-fake`) i atrapie Brokera:
//! agentka steruje oknem Notatnika, a wobec okien Alfy (WebView2, Broker-UI) — 0 skutków (także
//! w 200 losowych próbach); panel „Ekran" pokazuje akcje bez treści wpisywanej, zrzut tylko
//! w pamięci (nie w zdarzeniach); przejęcie sterowania wstrzymuje narzędzia GUI do oddania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use app_api::EventHub;
use app_api::dto::{AlfaEvent, GuiActionStatus};
use app_gui::{GuiMonitor, GuiPorts, gui_tools};
use compliance_contract::PathEnv;
use platform_contract::{ScreenRect, TargetGuard, WindowId};
use platform_fake::{FakeDesktop, FakeWindow};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, ToolStatus};
use watchdog_contract::ManualClock;

const ALFA_PID: u32 = 4_242;

struct World {
    desktop: Arc<FakeDesktop>,
    notepad: WindowId,
    alfa: Vec<WindowId>,
    tools: Vec<Arc<dyn Tool>>,
    monitor: Arc<GuiMonitor>,
    events: EventHub,
}

fn world() -> World {
    let guard = TargetGuard::baseline()
        .with_pids([ALFA_PID])
        .with_image_dirs([r"C:\Users\ala\AppData\Local\Alfa"]);
    let desktop = Arc::new(FakeDesktop::with_guard(guard));
    let rect = |x| ScreenRect::from_xywh(x, 0, 500, 400);
    let notepad = desktop.add_window(
        FakeWindow::new("Notatnik", r"C:\Windows\notepad.exe", rect(0)),
        true,
    );
    let alfa = vec![
        desktop.add_window(
            FakeWindow::new("Alfa", r"C:\Program Files\Alfa\alfa-desktop.exe", rect(520)),
            false,
        ),
        desktop.add_window(
            FakeWindow::new(
                "Broker",
                r"C:\Program Files\Alfa\alfa-broker-ui.exe",
                rect(1040),
            ),
            false,
        ),
        desktop.add_window(
            FakeWindow::new(
                "WebView",
                r"C:\Users\ala\AppData\Local\Alfa\versions\1\msedgewebview2.exe",
                rect(1300),
            ),
            false,
        ),
    ];
    let env = PathEnv::windows_profile(r"C:\Users\ala");
    let policy = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    let broker =
        Arc::new(FakeBroker::with(policy, env, Arc::new(ManualClock::new(1_000_000))).unwrap());
    let events = EventHub::start(Duration::from_millis(5));
    let monitor = Arc::new(GuiMonitor::new(Some(events.clone()), true));
    let ports = GuiPorts::from_one(desktop.clone(), true);
    let tools = gui_tools(&ports, broker.clone(), None, &monitor);
    for t in &tools {
        broker.script(&t.manifest().id, ScriptedDecision::Allow);
    }
    World {
        desktop,
        notepad,
        alfa,
        tools,
        monitor,
        events,
    }
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(300);
    c
}

impl World {
    fn tool(&self, name: &str) -> &Arc<dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.manifest().name == name)
            .unwrap()
    }

    fn alfa_records(&self) -> usize {
        self.desktop
            .records()
            .iter()
            .filter(|r| self.alfa.contains(&r.window))
            .count()
    }
}

#[tokio::test]
async fn agent_drives_notepad_but_never_alfa_windows() {
    let w = world();
    let mut rx = w.events.subscribe();
    let secret = "Hasło do banku 1234";
    let out = w
        .tool("input_type_text")
        .call(json!({"window": w.notepad.0, "text": secret}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(w.desktop.typed_text(w.notepad), secret);
    for alfa in &w.alfa {
        let denied = w
            .tool("input_type_text")
            .call(json!({"window": alfa.0, "text": "x"}), &ctx())
            .await;
        assert!(
            matches!(denied.status, ToolStatus::Denied { .. }),
            "{denied:?}"
        );
        let focus = w
            .tool("window_focus")
            .call(json!({"window": alfa.0}), &ctx())
            .await;
        assert!(matches!(focus.status, ToolStatus::Denied { .. }));
    }
    assert_eq!(w.alfa_records(), 0, "0 skutków w oknach Alfy/Brokera");
    let status = w.monitor.status();
    assert!(status.control.is_some(), "wskaźnik „agentka steruje”");
    assert_eq!(
        status.actions[status.actions.len() - 1].status,
        GuiActionStatus::Ok
    );
    let shown = serde_json::to_string(&status).unwrap();
    assert!(
        !shown.contains("banku"),
        "panel nie pokazuje wpisanej treści"
    );
    assert!(shown.contains("19 znaków"));
    tokio::time::sleep(Duration::from_millis(20)).await;
    let mut seen = String::new();
    while let Ok(batch) = rx.try_recv() {
        seen.push_str(&serde_json::to_string(&*batch).unwrap());
    }
    assert!(seen.contains("GuiActivity"));
    assert!(!seen.contains("banku"), "treść nie trafia do zdarzeń");
}

#[tokio::test]
async fn screenshot_stays_in_memory_and_out_of_events() {
    let w = world();
    let mut rx = w.events.subscribe();
    let out = w
        .tool("screen_capture")
        .call(json!({"target": "window", "window": w.notepad.0}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    let shot = w.monitor.screenshot().expect("ostatni zrzut");
    assert!(shot.data_url.starts_with("data:image/png;base64,"));
    assert!(shot.info.width > 0 && shot.info.height > 0);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let b64 = &shot.data_url["data:image/png;base64,".len()..];
    while let Ok(batch) = rx.try_recv() {
        let text = serde_json::to_string(&*batch).unwrap();
        assert!(
            !text.contains(&b64[..64.min(b64.len())]),
            "piksele nie w zdarzeniach"
        );
        assert!(!text.contains("data:image"));
        for e in batch.iter() {
            if let AlfaEvent::GuiActivity { status } = e {
                assert!(status.available);
            }
        }
    }
}

#[tokio::test]
async fn take_over_pauses_gui_tools_until_released() {
    let w = world();
    let first = w
        .tool("window_focus")
        .call(json!({"window": w.notepad.0}), &ctx())
        .await;
    assert!(first.is_ok(), "{first:?}");
    let sessions = w.monitor.take_over();
    assert_eq!(sessions.len(), 1, "sesja sterującej agentki do zatrzymania");
    let before = w.desktop.records().len();
    let paused = w
        .tool("input_type_text")
        .call(json!({"window": w.notepad.0, "text": "abc"}), &ctx())
        .await;
    assert!(matches!(paused.status, ToolStatus::Denied { .. }));
    assert!(paused.text.contains("przejął sterowanie"));
    assert_eq!(
        w.desktop.records().len(),
        before,
        "żadnego skutku po przejęciu"
    );
    assert!(w.monitor.status().taken_over);
    w.monitor.release();
    let again = w
        .tool("input_type_text")
        .call(json!({"window": w.notepad.0, "text": "abc"}), &ctx())
        .await;
    assert!(again.is_ok(), "{again:?}");
}

/// Deterministyczny generator (LCG) — powtarzalne „losowe" próby bez dodatkowych zależności.
fn lcg(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *seed >> 33
}

#[tokio::test]
async fn two_hundred_random_actions_never_touch_alfa() {
    let w = world();
    let mut seed = 7_u64;
    let mut targets = w.alfa.clone();
    targets.push(w.notepad);
    let names = [
        "input_type_text",
        "input_click",
        "input_keys",
        "window_focus",
        "window_state",
        "screen_capture",
    ];
    for _ in 0..200 {
        let win = targets[usize::try_from(lcg(&mut seed)).unwrap() % targets.len()];
        let name = names[usize::try_from(lcg(&mut seed)).unwrap() % names.len()];
        let args = match name {
            "input_type_text" => json!({"window": win.0, "text": "a"}),
            "input_click" => json!({"window": win.0, "x": 10, "y": 10}),
            "input_keys" => json!({"window": win.0, "keys": ["Ctrl+S"]}),
            "window_state" => json!({"window": win.0, "state": "minimized"}),
            "screen_capture" => json!({"target": "window", "window": win.0}),
            _ => json!({"window": win.0}),
        };
        let _ = w.tool(name).call(args, &ctx()).await;
    }
    assert_eq!(w.alfa_records(), 0);
    assert!(w.monitor.status().actions.len() <= app_gui::MAX_ACTIONS);
}
