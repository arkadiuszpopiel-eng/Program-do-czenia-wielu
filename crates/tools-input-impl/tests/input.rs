//! `tools-input` na wirtualnym pulpicie i atrapie Brokera: kontrakt, pisanie bez treści
//! w zdarzeniach, przerwanie przy fizycznym wejściu, skróty systemowe, kliknięcie zasłonięte
//! przez Broker-UI, limit tempa i właściwość: 0 zdarzeń w oknach Alfy/Brokera w 200 próbach.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use platform_contract::{ScreenRect, TargetGuard, UiaPattern, WindowId};
use platform_fake::{FakeDesktop, FakeElement, FakeWindow, ScriptEvent};
use proptest::prelude::*;
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{DenialReason, Tool, ToolCtx, ToolErrorKind, ToolStatus, Toolset};
use tools_input_contract::InputToolsConfig;
use tools_input_impl::{InputTools, InputToolsDeps};
use watchdog_contract::ManualClock;

fn broker() -> Arc<FakeBroker> {
    let env = PathEnv::windows_profile(r"C:\Users\ala");
    let policy = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    let b = Arc::new(FakeBroker::with(policy, env, Arc::new(ManualClock::new(1_000_000))).unwrap());
    for t in [
        "tools-input.type_text",
        "tools-input.keys",
        "tools-input.click",
        "tools-input.scroll",
    ] {
        b.script(t, ScriptedDecision::Allow);
    }
    b
}

fn tools(desktop: &Arc<FakeDesktop>, bus: Option<Arc<FakeBus>>, max_calls: u32) -> InputTools {
    InputTools::new(InputToolsDeps {
        desktop: desktop.clone(),
        uia: desktop.clone(),
        input: desktop.clone(),
        broker: broker(),
        config: InputToolsConfig {
            max_calls_per_minute: max_calls,
            ..InputToolsConfig::default()
        },
        bus: bus.map(|b| b as Arc<dyn core_bus_contract::EventBus>),
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

fn desktop() -> (Arc<FakeDesktop>, WindowId) {
    let d = Arc::new(FakeDesktop::new());
    let w = d.add_window(
        FakeWindow::new(
            "Notatnik",
            r"C:\Windows\notepad.exe",
            ScreenRect::from_xywh(0, 0, 800, 600),
        ),
        true,
    );
    (d, w)
}

#[tokio::test]
async fn contract_suite() {
    let (d, _) = desktop();
    tools_input_contract::contract_tests::run_all(&tools(&d, None, 1_000).tools()).await;
    assert!(d.records().is_empty());
}

#[tokio::test]
async fn typing_reaches_the_window_but_never_the_events() {
    let (d, w) = desktop();
    let other = d.add_window(
        FakeWindow::new(
            "Kalkulator",
            "calc.exe",
            ScreenRect::from_xywh(900, 0, 300, 300),
        ),
        true,
    );
    let bus = Arc::new(FakeBus::default());
    let t = tools(&d, Some(bus.clone()), 1_000);
    let secret = "Tajny tekst użytkownika 123";
    let out = tool(&t, "input_type_text")
        .call(json!({"window": w.0, "text": secret}), &ctx())
        .await;
    assert!(out.is_ok() && out.data["verified"] == true, "{out:?}");
    assert_eq!(d.typed_text(w), secret, "okno dostało fokus i tekst");
    assert!(d.typed_text(other).is_empty());
    let all: String = bus
        .recorded()
        .iter()
        .map(|e| serde_json::to_string(&e.payload).unwrap())
        .collect();
    assert!(!all.contains("Tajny"), "treść nie trafia do zdarzeń");
    assert_eq!(
        bus.recorded_of_kind(&EventKind::Custom("tool.gui.verify".into()))
            .len(),
        1
    );
    assert_eq!(out.untrusted, None);
}

#[tokio::test]
async fn physical_input_interrupts_and_user_activity_blocks() {
    let (d, w) = desktop();
    let t = tools(&d, None, 1_000);
    d.script_after(d.injected_batches() + 2, ScriptEvent::PhysicalInput);
    let out = tool(&t, "input_type_text")
        .call(json!({"window": w.0, "text": "x".repeat(200)}), &ctx())
        .await;
    assert_eq!(out.status, ToolStatus::Cancelled, "{out:?}");
    assert_eq!(out.data["interrupted_by_user"], true);
    assert_eq!(d.typed_text(w).len(), 32, "stop po dwóch paczkach");
    let again = tool(&t, "input_keys")
        .call(json!({"window": w.0, "keys": ["Ctrl+S"]}), &ctx())
        .await;
    assert_eq!(again.status, ToolStatus::Cancelled);
    assert_eq!(again.data["user_active"], true);
    d.advance(10_000);
    let ok = tool(&t, "input_keys")
        .call(json!({"window": w.0, "keys": ["Ctrl+S"]}), &ctx())
        .await;
    assert!(ok.is_ok(), "{ok:?}");
}

#[tokio::test]
async fn system_chords_and_covered_points_are_refused() {
    let (d, w) = desktop();
    let broker_ui = d.add_window(
        FakeWindow::new(
            "Zatwierdź",
            "alfa-broker-ui.exe",
            ScreenRect::from_xywh(100, 100, 300, 200),
        ),
        false,
    );
    let t = tools(&d, None, 1_000);
    let win_r = tool(&t, "input_keys")
        .call(json!({"window": w.0, "keys": ["Win+R"]}), &ctx())
        .await;
    assert!(matches!(
        win_r.status,
        ToolStatus::Denied {
            reason: DenialReason::Policy
        }
    ));
    let covered = tool(&t, "input_click")
        .call(json!({"window": w.0, "x": 150, "y": 150}), &ctx())
        .await;
    assert!(!covered.is_ok(), "{covered:?}");
    let outside = tool(&t, "input_click")
        .call(json!({"window": w.0, "x": 5000, "y": 5}), &ctx())
        .await;
    assert!(matches!(
        outside.status,
        ToolStatus::Denied {
            reason: DenialReason::Policy
        }
    ));
    let direct = tool(&t, "input_type_text")
        .call(json!({"window": broker_ui.0, "text": "tak"}), &ctx())
        .await;
    assert!(matches!(
        direct.status,
        ToolStatus::Denied {
            reason: DenialReason::KernelBlock { .. }
        }
    ));
    assert!(d.records().iter().all(|r| r.window != broker_ui));
    let clicked = tool(&t, "input_click")
        .call(json!({"window": w.0, "x": 700, "y": 500}), &ctx())
        .await;
    assert!(
        clicked.is_ok() && clicked.data["verified"] == true,
        "{clicked:?}"
    );
}

#[tokio::test]
async fn element_click_scroll_and_rate_limit() {
    let (d, w) = desktop();
    let button = d
        .add_element(
            w,
            FakeElement::new("Zapisz", "button", ScreenRect::from_xywh(40, 40, 100, 30))
                .patterns(&[UiaPattern::Invoke]),
        )
        .unwrap();
    let t = tools(&d, None, 3);
    let c = tool(&t, "input_click")
        .call(
            json!({"window": w.0, "element": button.to_string()}),
            &ctx(),
        )
        .await;
    assert!(c.is_ok(), "{c:?}");
    let s = tool(&t, "input_scroll")
        .call(json!({"window": w.0, "notches": -3}), &ctx())
        .await;
    assert!(s.is_ok(), "{s:?}");
    let foreign = tool(&t, "input_click")
        .call(json!({"window": w.0, "element": "w999:42.999.0"}), &ctx())
        .await;
    assert!(matches!(
        foreign.status,
        ToolStatus::Denied {
            reason: DenialReason::Policy
        }
    ));
    let limited = tool(&t, "input_keys")
        .call(json!({"window": w.0, "keys": ["Enter"]}), &ctx())
        .await;
    assert!(
        matches!(
            limited.status,
            ToolStatus::Denied {
                reason: DenialReason::Policy
            }
        ),
        "{limited:?}"
    );
    assert!(limited.text.contains("limit tempa"));
    let bad = tool(&t, "input_keys")
        .call(json!({"window": w.0, "keys": ["Ctrl+Foo"]}), &ctx())
        .await;
    assert_eq!(
        bad.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
}

const IMAGES: [&str; 6] = [
    "notepad.exe",
    "alfa-broker-ui.exe",
    "winword.exe",
    "alfa-desktop.exe",
    "alfa-watchdog.exe",
    "explorer.exe",
];

#[derive(Debug, Clone)]
enum Op {
    Type(usize, String),
    Keys(usize),
    Click(usize, i32, i32),
    Scroll(usize, i32, i32),
    Raise(usize, u64),
    Touch(u64),
    Wait,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..6usize, "[a-z ]{1,80}").prop_map(|(w, t)| Op::Type(w, t)),
        (0..6usize).prop_map(Op::Keys),
        (0..6usize, -50..700i32, -50..500i32).prop_map(|(w, x, y)| Op::Click(w, x, y)),
        (0..6usize, 0..600i32, 0..400i32).prop_map(|(w, x, y)| Op::Scroll(w, x, y)),
        (0..6usize, 1..5u64).prop_map(|(w, n)| Op::Raise(w, n)),
        (1..5u64).prop_map(Op::Touch),
        Just(Op::Wait),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn zero_input_events_reach_protected_windows(
        layout in prop::collection::vec((0..6usize, 0..1200i32, 0..600i32, any::<bool>()), 2..7),
        ops in prop::collection::vec(op(), 1..14),
    ) {
        let d = Arc::new(FakeDesktop::new());
        let ids: Vec<WindowId> = layout
            .iter()
            .enumerate()
            .map(|(i, (img, x, y, focus))| d.add_window(FakeWindow::new(&format!("o{i}"), IMAGES[*img], ScreenRect::from_xywh(*x, *y, 640, 480)), *focus))
            .collect();
        let t = tools(&d, None, 10_000);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            for o in &ops {
                let id = |w: &usize| ids[w % ids.len()].0;
                let (name, args) = match o {
                    Op::Type(w, text) => ("input_type_text", json!({"window": id(w), "text": text})),
                    Op::Keys(w) => ("input_keys", json!({"window": id(w), "keys": ["Ctrl+A", "Delete"]})),
                    Op::Click(w, x, y) => ("input_click", json!({"window": id(w), "x": x, "y": y})),
                    Op::Scroll(w, x, y) => ("input_scroll", json!({"window": id(w), "x": x, "y": y, "notches": 2})),
                    Op::Raise(w, n) => {
                        d.script_after(d.injected_batches() + n, ScriptEvent::Raise(ids[w % ids.len()]));
                        continue;
                    }
                    Op::Touch(n) => {
                        d.script_after(d.injected_batches() + n, ScriptEvent::PhysicalInput);
                        continue;
                    }
                    Op::Wait => {
                        d.advance(5_000);
                        continue;
                    }
                };
                let _ = tool(&t, name).call(args, &ctx()).await;
            }
        });
        let guard = TargetGuard::baseline();
        let bad: Vec<_> = d.records().into_iter().filter(|r| guard.is_protected(r.pid, &r.image)).collect();
        prop_assert!(bad.is_empty(), "{bad:?}");
    }
}
