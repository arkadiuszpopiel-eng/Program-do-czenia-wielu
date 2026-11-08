//! `tools-screen` na wirtualnym pulpicie i atrapie Brokera: kontrakt, maskowanie (Alfa, hasła,
//! aplikacje dostawców), taint, odmowa zrzutu okna chronionego, brak pikseli w zdarzeniach,
//! limit rozmiaru, czarna klatka.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use core_bus_fake::FakeBus;
use platform_contract::{MASK_COLOR, ScreenRect, WindowId};
use platform_fake::{FakeDesktop, FakeElement, FakeWindow, PASSWORD_COLOR};
use safety_broker_contract::{Broker, Holder, KernelPolicy, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{DenialReason, Tool, ToolCtx, ToolErrorKind, ToolStatus, Toolset};
use tools_screen_contract::ScreenToolsConfig;
use tools_screen_impl::{ScreenTools, ScreenToolsDeps};
use watchdog_contract::ManualClock;

struct Env {
    desktop: Arc<FakeDesktop>,
    broker: Arc<FakeBroker>,
    bus: Arc<FakeBus>,
    notepad: WindowId,
    alfa: WindowId,
}

fn env() -> Env {
    let desktop = Arc::new(FakeDesktop::new());
    let notepad = desktop.add_window(
        FakeWindow::new(
            "Notatnik",
            "notepad.exe",
            ScreenRect::from_xywh(0, 0, 900, 700),
        ),
        true,
    );
    desktop.add_element(
        notepad,
        FakeElement::new("Hasło", "edit", ScreenRect::from_xywh(50, 50, 200, 25)).password(),
    );
    let alfa = desktop.add_window(
        FakeWindow::new(
            "Alfa",
            "alfa-desktop.exe",
            ScreenRect::from_xywh(1000, 0, 900, 700),
        ),
        false,
    );
    desktop.add_window(
        FakeWindow::new(
            "Claude",
            "claude.exe",
            ScreenRect::from_xywh(0, 750, 500, 300),
        ),
        false,
    );
    let policy = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(
            policy,
            PathEnv::windows_profile(r"C:\Users\ala"),
            Arc::new(ManualClock::new(1_000_000)),
        )
        .unwrap(),
    );
    broker.script("tools-screen.capture", ScriptedDecision::Allow);
    Env {
        desktop,
        broker,
        bus: Arc::new(FakeBus::default()),
        notepad,
        alfa,
    }
}

fn tool(e: &Env, config: ScreenToolsConfig) -> Arc<dyn Tool> {
    ScreenTools::new(ScreenToolsDeps {
        desktop: e.desktop.clone(),
        capture: e.desktop.clone(),
        broker: e.broker.clone(),
        config,
        bus: Some(e.bus.clone()),
    })
    .tools()
    .remove(0)
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(300);
    c
}

#[tokio::test]
async fn contract_suite() {
    let e = env();
    let t = ScreenTools::new(ScreenToolsDeps {
        desktop: e.desktop.clone(),
        capture: e.desktop.clone(),
        broker: e.broker.clone(),
        config: ScreenToolsConfig::default(),
        bus: None,
    });
    tools_screen_contract::contract_tests::run_all(&t.tools()).await;
}

#[tokio::test]
async fn monitor_capture_is_masked_tainted_and_pixel_free_in_events() {
    let e = env();
    // Atrapa koduje PNG bez kompresji (~8 MB dla 1920×1080) — limit podniesiony tylko tutaj.
    let config = ScreenToolsConfig {
        max_png_bytes: 16 * 1024 * 1024,
        ..ScreenToolsConfig::default()
    };
    let out = tool(&e, config)
        .call(json!({"target": "monitor", "max_side": 1920}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(out.untrusted, Some(TaintSource::Screen));
    assert_eq!(out.images.len(), 1);
    assert!(out.images[0].data_base64.starts_with("iVBORw0KGgo"), "PNG");
    let img = e.desktop.last_capture().unwrap();
    assert!(
        img.pixels.chunks_exact(4).all(|p| p != PASSWORD_COLOR),
        "pole hasła zamaskowane"
    );
    assert_eq!(
        img.pixel(1500, 300),
        Some(MASK_COLOR),
        "okno Alfy zamaskowane"
    );
    assert_eq!(
        img.pixel(200, 900),
        Some(MASK_COLOR),
        "aplikacja dostawcy zamaskowana"
    );
    let reasons: Vec<String> = out.data["masked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["reason"].as_str().unwrap().to_owned())
        .collect();
    for r in ["protected_window", "masked_app", "password_field"] {
        assert!(reasons.iter().any(|x| x == r), "{r}: {reasons:?}");
    }
    assert!(e.broker.session_security(&"s1".into()).tainted);
    let events: String = e
        .bus
        .recorded()
        .iter()
        .map(|ev| serde_json::to_string(&ev.payload).unwrap())
        .collect();
    assert_eq!(
        e.bus
            .recorded_of_kind(&core_bus_contract::EventKind::Custom(
                "tool.screen.capture".into()
            ))
            .len(),
        1
    );
    assert!(!events.contains("iVBOR"), "piksele nie trafiają do zdarzeń");
}

#[tokio::test]
async fn window_capture_protected_denied_scaled_and_limited() {
    let e = env();
    let t = tool(&e, ScreenToolsConfig::default());
    let denied = t
        .call(json!({"target": "window", "window": e.alfa.0}), &ctx())
        .await;
    assert!(
        matches!(
            denied.status,
            ToolStatus::Denied {
                reason: DenialReason::KernelBlock { .. }
            }
        ),
        "{denied:?}"
    );
    let ok = t
        .call(
            json!({"target": "window", "window": e.notepad.0, "max_side": 450}),
            &ctx(),
        )
        .await;
    assert!(ok.is_ok());
    assert_eq!(ok.data["width"], 450);
    assert_eq!(ok.data["scale"], 2.0);
    let tiny = tool(
        &e,
        ScreenToolsConfig {
            max_png_bytes: 100,
            ..ScreenToolsConfig::default()
        },
    );
    let big = tiny.call(json!({"target": "monitor"}), &ctx()).await;
    assert_eq!(
        big.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Io
        }
    );
    let region = t
        .call(
            json!({"target": "region", "x": 0, "y": 0, "width": 100, "height": 100}),
            &ctx(),
        )
        .await;
    assert!(region.is_ok(), "{region:?}");
}

#[tokio::test]
async fn black_frame_is_reported() {
    let e = env();
    let drm = e.desktop.add_window(
        FakeWindow {
            capture_blocked: true,
            ..FakeWindow::new(
                "Film",
                "player.exe",
                ScreenRect::from_xywh(100, 100, 300, 200),
            )
        },
        false,
    );
    let out = tool(&e, ScreenToolsConfig::default())
        .call(json!({"target": "window", "window": drm.0}), &ctx())
        .await;
    assert!(out.is_ok() && out.data["black_frame"] == true);
    assert!(out.text.contains("czarna"));
}
