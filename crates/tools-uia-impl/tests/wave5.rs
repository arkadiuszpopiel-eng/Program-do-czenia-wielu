//! Fala 5: `uia_tree` zgłasza `sparse` dla okna z samą ramą (F6-03 — rama i pasek tytułu nie
//! liczą się do progu) i odmawia odczytu/akcji w oknach menedżerów haseł (PT-25).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use platform_contract::{ScreenRect, UiaPattern, WindowId};
use platform_fake::{FakeDesktop, FakeElement, FakeWindow};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{DenialReason, Tool, ToolCtx, ToolStatus, Toolset};
use tools_uia_contract::UiaToolsConfig;
use tools_uia_impl::{UiaTools, UiaToolsDeps};
use watchdog_contract::ManualClock;

fn broker() -> Arc<FakeBroker> {
    let env = PathEnv::windows_profile(r"C:\Users\ala");
    let policy = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    let b = Arc::new(FakeBroker::with(policy, env, Arc::new(ManualClock::new(1_000_000))).unwrap());
    for t in [
        "tools-uia.tree",
        "tools-uia.find",
        "tools-uia.read_text",
        "tools-uia.act",
    ] {
        b.script(t, ScriptedDecision::Allow);
    }
    b
}

fn tool(desktop: &Arc<FakeDesktop>, name: &str) -> Arc<dyn Tool> {
    let t = UiaTools::new(UiaToolsDeps {
        desktop: desktop.clone(),
        uia: desktop.clone(),
        broker: broker(),
        config: UiaToolsConfig::default(),
        bus: None,
    });
    t.tools()
        .into_iter()
        .find(|x| x.manifest().name == name)
        .unwrap()
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(300);
    c
}

/// Okno z ramą Win32 (okno, pasek tytułu, menu systemowe, 3 przyciski) i jedną płaszczyzną.
fn frame_window(d: &FakeDesktop, image: &str) -> WindowId {
    let r = ScreenRect::from_xywh(0, 0, 100, 30);
    let w = d.add_window(
        FakeWindow::new("Płótno", image, ScreenRect::from_xywh(0, 0, 800, 600)),
        true,
    );
    for (depth, role, id) in [
        (0, "window", ""),
        (1, "title_bar", "TitleBar"),
        (2, "menu_bar", "SystemMenuBar"),
        (3, "menu_item", "Item 1"),
        (2, "button", "Minimize"),
        (2, "button", "Maximize"),
        (2, "button", "Close"),
        (1, "pane", "Canvas"),
    ] {
        d.add_element(
            w,
            FakeElement::new(id, role, r)
                .depth(depth)
                .automation_id(id)
                .patterns(&[UiaPattern::Invoke]),
        );
    }
    w
}

#[tokio::test]
async fn frame_only_window_is_reported_sparse_with_vision_hint() {
    let d = Arc::new(FakeDesktop::new());
    let w = frame_window(&d, r"C:\Apps\paint.exe");
    let out = tool(&d, "uia_tree")
        .call(json!({"window": w.0}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(out.data["nodes"].as_array().unwrap().len(), 8);
    assert_eq!(out.data["sparse"], true, "8 węzłów, ale klient = 1");
    assert!(out.text.contains("screen_capture"), "{}", out.text);
}

#[tokio::test]
async fn password_manager_windows_are_refused_for_reads_and_actions() {
    let d = Arc::new(FakeDesktop::new());
    for image in [
        r"C:\Program Files\KeePass Password Safe 2\KeePass.exe",
        r"C:\Users\ala\AppData\Local\1Password\app\8\1Password.exe",
        r"C:\Users\ala\AppData\Local\Programs\Bitwarden\Bitwarden.exe",
        r"C:\Windows\System32\CredentialUIBroker.exe",
    ] {
        let w = frame_window(&d, image);
        let entry = d
            .add_element(
                w,
                FakeElement::new(
                    "bank.example — ala",
                    "list_item",
                    ScreenRect::from_xywh(10, 40, 200, 20),
                )
                .patterns(&[UiaPattern::Invoke, UiaPattern::Value])
                .value("ala@example.com")
                .text("PIN 4321"),
            )
            .unwrap();
        for (name, args) in [
            ("uia_tree", json!({"window": w.0})),
            ("uia_find", json!({"window": w.0, "name_contains": "bank"})),
            ("uia_read_text", json!({"element": entry.to_string()})),
            (
                "uia_act",
                json!({"element": entry.to_string(), "action": "invoke"}),
            ),
        ] {
            let out = tool(&d, name).call(args, &ctx()).await;
            assert!(
                matches!(
                    out.status,
                    ToolStatus::Denied {
                        reason: DenialReason::KernelBlock { .. }
                    }
                ),
                "{image} {name}: {out:?}"
            );
            assert!(
                !out.text.contains("bank.example") && !out.text.contains("PIN"),
                "{image} {name}: wyciek treści"
            );
        }
    }
    assert!(d.records().is_empty());
}
