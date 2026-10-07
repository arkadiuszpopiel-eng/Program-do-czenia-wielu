//! Wizja i multimedia w aplikacji: `MediaPorts` składa pięć narzędzi (`vision_*`, `media_*`),
//! przysługują one tylko roli Wykonawczyni (grupy `vision`, `media`), prywatność opisu obrazu
//! pochodzi z katalogu sesji (prywatna albo nieznana — tylko lokalnie; bez Routera — odmowa,
//! zanim obraz wyjdzie), a bez ffmpeg i wyjścia audio narzędzia odpowiadają czytelnym błędem.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use app_agents::{MediaPorts, ffmpeg_path};
use app_api::AppPaths;
use app_gui::GuiPorts;
use compliance_contract::PathEnv;
use core_bus_contract::EventBus;
use core_bus_fake::FakeBus;
use personas_contract::builtin_roles;
use platform_contract::ScreenRect;
use platform_fake::{FakeDesktop, FakeFs, FakeWindow};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use scheduler_lite_fake::FakeScheduler;
use serde_json::json;
use sessions_contract::{NewSession, PrivacyTag, SessionCatalog};
use sessions_fake::FakeSessions;
use tools_common_contract::{DenialReason, Tool, ToolCtx, ToolErrorKind, ToolStatus};
use undo_journal_contract::{Journal, MemStore, UndoLimits};
use watchdog_contract::ManualClock;

const HOME: &str = "/Users/ala";

struct H {
    tools: Vec<Arc<dyn Tool>>,
    sessions: Arc<FakeSessions>,
}

fn harness() -> H {
    let desktop = Arc::new(FakeDesktop::new());
    desktop.add_window(
        FakeWindow::new(
            "Notatnik",
            "notepad.exe",
            ScreenRect::from_xywh(0, 0, 400, 300),
        ),
        true,
    );
    let gui = GuiPorts::from_one(desktop, true);
    let sessions = Arc::new(FakeSessions::new());
    let paths = AppPaths::under(&PathBuf::from("/alfa-test"));
    let media = MediaPorts::new(
        &gui,
        sessions.clone(),
        Arc::new(FakeScheduler::new()),
        &paths,
    )
    .with(None, None)
    .exec(None);
    let fs = Arc::new(FakeFs::new());
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(
            policy,
            PathEnv::windows_profile(HOME),
            Arc::new(ManualClock::new(1_000_000)),
        )
        .unwrap(),
    );
    for t in [
        "tools-vision.describe",
        "tools-media.convert",
        "tools-media.play",
    ] {
        broker.script(t, ScriptedDecision::Allow);
    }
    let tick = Arc::new(AtomicU64::new(1_000));
    let clock = move || tick.fetch_add(1, Ordering::SeqCst);
    let journal = Arc::new(
        Journal::open(
            fs.clone(),
            Arc::new(MemStore::default()),
            UndoLimits::default(),
            Arc::new(clock),
            1,
        )
        .unwrap(),
    );
    let bus: Arc<dyn EventBus> = Arc::new(FakeBus::default());
    let tools = media.tools(broker, journal, fs, &bus);
    assert!(format!("{media:?}").contains("ffmpeg"));
    assert!(ffmpeg_path(&paths).ends_with(if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }));
    H { tools, sessions }
}

fn tool(h: &H, name: &str) -> Arc<dyn Tool> {
    h.tools
        .iter()
        .find(|t| t.manifest().name == name)
        .unwrap()
        .clone()
}

fn ctx(session: &str) -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent(session, "delta")).with_workdir("/Users/ala/Muzyka");
    c.approval_timeout = Duration::from_millis(300);
    c
}

#[test]
fn five_tools_only_for_the_operator() {
    let h = harness();
    let names: Vec<String> = h.tools.iter().map(|t| t.manifest().name.clone()).collect();
    assert_eq!(
        names,
        [
            "vision_ocr",
            "vision_describe",
            "media_info",
            "media_convert",
            "media_play"
        ]
    );
    for role in builtin_roles() {
        let allowed: Vec<&str> = h
            .tools
            .iter()
            .map(|t| t.manifest())
            .filter(|m| m.allowed_for(&role.tools, role.read_only))
            .map(|m| m.name.as_str())
            .collect();
        if role.id.as_str() == "operator" {
            assert_eq!(allowed.len(), 5, "{allowed:?}");
        } else {
            assert!(allowed.is_empty(), "{}: {allowed:?}", role.id.as_str());
        }
    }
}

#[tokio::test]
async fn describe_without_router_refuses_by_session_privacy() {
    let h = harness();
    let normal = h
        .sessions
        .create_session(NewSession {
            title: "Zwykła".into(),
            privacy: PrivacyTag::Normal,
            ..NewSession::default()
        })
        .unwrap();
    let private = h
        .sessions
        .create_session(NewSession {
            title: "Prywatna".into(),
            privacy: PrivacyTag::Private,
            ..NewSession::default()
        })
        .unwrap();
    let describe = tool(&h, "vision_describe");
    let args = json!({"source": "screen", "target": "region", "x": 0, "y": 0, "width": 200, "height": 100});
    let out = describe.call(args.clone(), &ctx(normal.id.as_str())).await;
    assert_eq!(
        out.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Unsupported
        },
        "{out:?}"
    );
    for session in [private.id.as_str(), "nieznana"] {
        let out = describe.call(args.clone(), &ctx(session)).await;
        assert_eq!(
            out.status,
            ToolStatus::Denied {
                reason: DenialReason::Policy
            },
            "{session}: {out:?}"
        );
    }
}

#[tokio::test]
async fn without_ffmpeg_and_speaker_tools_explain_why() {
    let h = harness();
    let convert = tool(&h, "media_convert")
        .call(json!({"path": "a.wav", "format": "mp3"}), &ctx("s1"))
        .await;
    assert_eq!(
        convert.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Unsupported
        }
    );
    assert!(convert.text.contains("ffmpeg"), "{}", convert.text);
}
