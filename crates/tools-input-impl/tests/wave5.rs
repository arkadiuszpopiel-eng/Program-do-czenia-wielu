//! Fala 5, PT-25: wejście syntetyczne nie trafia do menedżerów haseł ani okna poświadczeń
//! Windows — wpisywanie, skróty, kliknięcia (także w punkt zasłonięty takim oknem) są odmawiane
//! przed jakimkolwiek zdarzeniem (dotąd te aplikacje były tylko maskowane na zrzutach).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use platform_contract::ScreenRect;
use platform_fake::{FakeDesktop, FakeWindow};
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

fn tool(desktop: &Arc<FakeDesktop>, name: &str) -> Arc<dyn Tool> {
    let t = InputTools::new(InputToolsDeps {
        desktop: desktop.clone(),
        uia: desktop.clone(),
        input: desktop.clone(),
        broker: broker(),
        config: InputToolsConfig::default(),
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

#[tokio::test]
async fn input_never_reaches_password_managers() {
    for image in [
        r"C:\Program Files\KeePass Password Safe 2\KeePass.exe",
        r"C:\Program Files\KeePassXC\KeePassXC.exe",
        r"C:\Users\ala\AppData\Local\1Password\app\8\1Password.exe",
        r"C:\Users\ala\AppData\Local\Programs\Bitwarden\Bitwarden.exe",
        r"C:\Windows\System32\CredentialUIBroker.exe",
    ] {
        let d = Arc::new(FakeDesktop::new());
        let notepad = d.add_window(
            FakeWindow::new(
                "Notatnik",
                r"C:\Windows\notepad.exe",
                ScreenRect::from_xywh(0, 0, 800, 600),
            ),
            false,
        );
        let vault = d.add_window(
            FakeWindow::new(
                "Baza haseł",
                image,
                ScreenRect::from_xywh(100, 100, 300, 200),
            ),
            true,
        );
        for (name, args) in [
            (
                "input_type_text",
                json!({"window": vault.0, "text": "hasło123"}),
            ),
            ("input_keys", json!({"window": vault.0, "keys": ["Ctrl+C"]})),
            (
                "input_click",
                json!({"window": vault.0, "x": 150, "y": 150}),
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
        }
        // Punkt okna zwykłego zasłonięty menedżerem haseł — też odmowa.
        let covered = tool(&d, "input_click")
            .call(json!({"window": notepad.0, "x": 200, "y": 200}), &ctx())
            .await;
        assert!(!covered.is_ok(), "{image}: {covered:?}");
        assert!(d.typed_text(vault).is_empty());
        assert!(
            d.records().iter().all(|r| r.window != vault),
            "{image}: zero zdarzeń w oknie menedżera haseł"
        );
    }
}
