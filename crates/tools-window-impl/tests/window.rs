//! `tools-window` na wirtualnym pulpicie i atrapie Brokera (prawdziwy silnik reguł Jądra):
//! kontrakt, lista bez okien Alfy i z taintem, odmowa wobec okien chronionych (0 skutków),
//! zmiana z weryfikacją, pytanie Brokera na L3.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use platform_contract::{DesktopPort, ScreenRect, WindowId, WindowState};
use platform_fake::{FakeDesktop, FakeWindow};
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{ApprovalDecision, Broker, Holder, KernelPolicy, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{DenialReason, Tool, ToolCtx, ToolStatus, Toolset};
use tools_window_impl::{WindowTools, WindowToolsDeps};
use watchdog_contract::ManualClock;

struct Env {
    desktop: Arc<FakeDesktop>,
    broker: Arc<FakeBroker>,
    bus: Arc<FakeBus>,
    tools: WindowTools,
    notepad: WindowId,
    alfa: WindowId,
}

fn env(allow: bool) -> Env {
    let desktop = Arc::new(FakeDesktop::new());
    let alfa = desktop.add_window(
        FakeWindow::new(
            "Alfa — rozmowa",
            "alfa-desktop.exe",
            ScreenRect::from_xywh(900, 0, 900, 900),
        ),
        false,
    );
    let notepad = desktop.add_window(
        FakeWindow::new(
            "Notatki sk-ant-api03-ABCDEFGHIJKLMNOPQRSTUV",
            r"C:\Windows\notepad.exe",
            ScreenRect::from_xywh(0, 0, 800, 600),
        ),
        true,
    );
    let env = PathEnv::windows_profile(r"C:\Users\ala");
    let policy = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    let broker =
        Arc::new(FakeBroker::with(policy, env, Arc::new(ManualClock::new(1_000_000))).unwrap());
    if allow {
        for t in [
            "tools-window.list",
            "tools-window.focus",
            "tools-window.move",
            "tools-window.state",
        ] {
            broker.script(t, ScriptedDecision::Allow);
        }
    }
    let bus = Arc::new(FakeBus::default());
    let tools = WindowTools::new(WindowToolsDeps {
        desktop: desktop.clone(),
        broker: broker.clone(),
        bus: Some(bus.clone()),
    });
    Env {
        desktop,
        broker,
        bus,
        tools,
        notepad,
        alfa,
    }
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(300);
    c
}

fn tool(e: &Env, name: &str) -> Arc<dyn Tool> {
    e.tools
        .tools()
        .into_iter()
        .find(|t| t.manifest().name == name)
        .unwrap()
}

#[tokio::test]
async fn contract_suite() {
    let e = env(true);
    tools_window_contract::contract_tests::run_all(&e.tools.tools()).await;
    assert!(
        e.desktop.records().is_empty(),
        "żadnych skutków z testów kontraktowych"
    );
}

#[tokio::test]
async fn list_hides_protected_redacts_and_taints() {
    let e = env(true);
    let out = tool(&e, "window_list").call(json!({}), &ctx()).await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(out.untrusted, Some(TaintSource::Screen));
    assert_eq!(out.data["hidden_protected"], 1);
    assert_eq!(out.data["windows"].as_array().unwrap().len(), 1);
    assert!(!out.text.contains("Alfa — rozmowa") && !out.text.contains("sk-ant"));
    assert!(e.broker.session_security(&"s1".into()).tainted);
    assert_eq!(
        e.bus
            .recorded_of_kind(&EventKind::Custom("tool.window.list".into()))
            .len(),
        1
    );
}

#[tokio::test]
async fn protected_windows_are_denied_before_the_broker() {
    let e = env(true);
    for (name, args) in [
        ("window_focus", json!({"window": e.alfa.0})),
        (
            "window_move",
            json!({"window": e.alfa.0, "x": 0, "y": 0, "width": 300, "height": 300}),
        ),
        (
            "window_state",
            json!({"window": e.alfa.0, "state": "minimized"}),
        ),
    ] {
        let out = tool(&e, name).call(args, &ctx()).await;
        assert!(
            matches!(
                out.status,
                ToolStatus::Denied {
                    reason: DenialReason::KernelBlock {
                        rule: KernelRule::GuiControlOfKernelProcess
                    }
                }
            ),
            "{name}: {out:?}"
        );
    }
    assert!(e.desktop.records().is_empty());
}

#[tokio::test]
async fn move_and_state_are_verified() {
    let e = env(true);
    let out = tool(&e, "window_move")
        .call(
            json!({"window": e.notepad.0, "x": 20, "y": 30, "width": 640, "height": 480}),
            &ctx(),
        )
        .await;
    assert!(out.is_ok() && out.data["verified"] == true, "{out:?}");
    assert_eq!(out.data["before"]["width"], 800);
    assert_eq!(
        e.desktop.window(e.notepad).unwrap().rect,
        ScreenRect::from_xywh(20, 30, 640, 480)
    );
    let out = tool(&e, "window_state")
        .call(json!({"window": e.notepad.0, "state": "minimized"}), &ctx())
        .await;
    assert!(out.is_ok() && out.data["verified"] == true);
    assert_eq!(
        e.desktop.window(e.notepad).unwrap().state,
        WindowState::Minimized
    );
    let out = tool(&e, "window_focus")
        .call(json!({"window": e.notepad.0}), &ctx())
        .await;
    assert!(out.is_ok() && out.data["verified"] == true);
    let verifies = e
        .bus
        .recorded_of_kind(&EventKind::Custom("tool.gui.verify".into()));
    assert_eq!(
        verifies.len(),
        3,
        "krok weryfikacji po każdej zmianie (F6-04)"
    );
    let bad = tool(&e, "window_move")
        .call(
            json!({"window": e.notepad.0, "x": 9000, "y": 0, "width": 300, "height": 300}),
            &ctx(),
        )
        .await;
    assert!(matches!(
        bad.status,
        ToolStatus::Denied {
            reason: DenialReason::Policy
        }
    ));
}

#[tokio::test]
async fn l3_asks_and_denial_leaves_window_untouched() {
    let e = env(false);
    e.broker
        .script("tools-window.state", ScriptedDecision::NeedsApproval);
    let b = e.broker.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        b.auto_approve(ApprovalDecision::Deny).await
    });
    let mut c = ctx();
    c.approval_timeout = Duration::from_secs(5);
    let out = tool(&e, "window_state")
        .call(json!({"window": e.notepad.0, "state": "minimized"}), &c)
        .await;
    assert!(
        matches!(
            out.status,
            ToolStatus::Denied {
                reason: DenialReason::OwnerDenied { .. }
            }
        ),
        "{out:?}"
    );
    assert_eq!(
        e.desktop.window(e.notepad).unwrap().state,
        WindowState::Normal
    );
}

mod property {
    use super::*;
    use platform_contract::TargetGuard;
    use proptest::prelude::*;

    const IMAGES: [&str; 5] = [
        "notepad.exe",
        "alfa-broker-ui.exe",
        "alfa.exe",
        "excel.exe",
        "alfa-uiaccess-helper.exe",
    ];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn zero_window_changes_on_protected_windows(
            layout in prop::collection::vec((0..5usize, any::<bool>()), 2..6),
            ops in prop::collection::vec((0..3usize, 0..6usize, 0..1200i32, 0..600i32), 1..10),
        ) {
            let e = env(true);
            let ids: Vec<WindowId> = layout
                .iter()
                .enumerate()
                .map(|(i, (img, focus))| e.desktop.add_window(FakeWindow::new(&format!("o{i}"), IMAGES[*img], ScreenRect::from_xywh(0, 0, 640, 480)), *focus))
                .collect();
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async {
                for (kind, w, x, y) in &ops {
                    let id = ids[w % ids.len()].0;
                    let (name, args) = match kind {
                        0 => ("window_focus", json!({"window": id})),
                        1 => ("window_move", json!({"window": id, "x": x, "y": y, "width": 400, "height": 300})),
                        _ => ("window_state", json!({"window": id, "state": "maximized"})),
                    };
                    let _ = tool(&e, name).call(args, &ctx()).await;
                }
            });
            let guard = TargetGuard::baseline();
            let bad: Vec<_> = e.desktop.records().into_iter().filter(|r| guard.is_protected(r.pid, &r.image)).collect();
            prop_assert!(bad.is_empty(), "{bad:?}");
        }
    }
}
