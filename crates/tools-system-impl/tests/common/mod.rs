//! Wspólny harness testów `tools-system`: `FakeSys` (drzewo procesów z Alfą i jej WebView2,
//! Broker, watchdog, procesy użytkownika i cudze), Broker z prawdziwym silnikiem i skryptem
//! decyzji, magistrala-atrapa.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use core_bus_fake::FakeBus;
use platform_apps_contract::{EnvScope, ProcessDetails, ServiceState};
use platform_apps_fake::{FakeSys, fake_process};
use platform_contract::TargetGuard;
use safety_broker_contract::{Broker, Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use tools_common_contract::{Tool, ToolCtx, Toolset};
use tools_system_contract::SystemToolsConfig;
use tools_system_impl::{SystemTools, SystemToolsDeps};
use watchdog_contract::ManualClock;

pub const HOME: &str = "/Users/ala";
/// Launcher Alfy (PID z listy strażnika) i jego potomek.
pub const LAUNCHER: u32 = 900;
pub const NOTEPAD: u32 = 4321;

pub struct H {
    pub sys: Arc<FakeSys>,
    pub broker: Arc<FakeBroker>,
    pub bus: Arc<FakeBus>,
    pub tools: SystemTools,
}

pub fn other_user(pid: u32, image: &str) -> ProcessDetails {
    let mut p = fake_process(pid, 1, image);
    p.entry.own = Some(false);
    p
}

pub fn sys() -> Arc<FakeSys> {
    let me = std::process::id();
    let guard = TargetGuard::baseline()
        .with_pids([LAUNCHER])
        .with_image_dirs([r"C:\Users\ala\AppData\Local\Alfa"]);
    let s = Arc::new(FakeSys::new(guard));
    s.add_process(fake_process(me, LAUNCHER, "alfa.exe"));
    s.add_process(fake_process(5000, me, "msedgewebview2.exe"));
    s.add_process(fake_process(LAUNCHER, 1, "alfa-launcher.exe"));
    s.add_process(fake_process(901, LAUNCHER, "llama-server.exe"));
    s.add_process(fake_process(902, 1, "alfa-broker.exe"));
    s.add_process(fake_process(903, 1, "alfa-watchdog.exe"));
    s.add_process(fake_process(904, 1, "ALFA-B~1.EXE"));
    let mut in_alfa_dir = fake_process(905, 1, "helper.exe");
    in_alfa_dir.path = Some(r"C:\Users\ala\AppData\Local\Alfa\v2\helper.exe".into());
    s.add_process(in_alfa_dir);
    s.add_process(fake_process(NOTEPAD, 1, "notepad.exe"));
    s.add_process(fake_process(700, 1, "lsass.exe"));
    s.add_process(other_user(6000, "chrome.exe"));
    let mut admin = fake_process(6001, 1, "regedit.exe");
    admin.elevated = Some(true);
    s.add_process(admin);
    s.add_service("Spooler", ServiceState::Running);
    s.add_service("WinDefend", ServiceState::Stopped);
    s.add_service("AlfaBroker", ServiceState::Running);
    s.add_service("wuauserv", ServiceState::Running);
    s.add_service("W32Time", ServiceState::Stopped);
    s.deny_service("W32Time");
    s.set_var(EnvScope::User, "PATH", r"C:\Users\ala\bin");
    s.set_var(
        EnvScope::User,
        "OPENAI_API_KEY",
        "sk-proj-abcdefghijklmnopqrstuvwx",
    );
    s.set_var(
        EnvScope::User,
        "MOJA_NOTATKA",
        "token=ghp_abcdefghijklmnopqrstuvwxyz0123",
    );
    s
}

pub fn harness_with(sys: Arc<FakeSys>, allow: bool) -> H {
    let env = PathEnv::windows_profile(HOME);
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker =
        Arc::new(FakeBroker::with(policy, env, Arc::new(ManualClock::new(1_000_000))).unwrap());
    if allow {
        for t in [
            "processes",
            "process_info",
            "process_kill",
            "services",
            "service_control",
            "events",
            "env",
            "env_set",
            "status",
        ] {
            broker.script(&format!("tools-system.{t}"), ScriptedDecision::Allow);
        }
    }
    let bus = Arc::new(FakeBus::default());
    let tools = SystemTools::new(SystemToolsDeps {
        sys: sys.clone(),
        power: None,
        desktop: None,
        hardware: None,
        broker: broker.clone(),
        config: SystemToolsConfig::default(),
        bus: Some(bus.clone()),
    });
    H {
        sys,
        broker,
        bus,
        tools,
    }
}

pub fn harness(allow: bool) -> H {
    harness_with(sys(), allow)
}

pub fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(200);
    c
}

impl H {
    pub fn tool(&self, n: &str) -> Arc<dyn Tool> {
        self.tools
            .tools()
            .into_iter()
            .find(|t| t.manifest().name == n)
            .unwrap()
    }

    /// Zdolności, na które Broker wydał tokeny (`rodzina`, zakres).
    pub fn issued(&self) -> Vec<(String, serde_json::Value)> {
        self.broker
            .audit_events()
            .iter()
            .filter(|e| e.kind.as_str() == "broker.token.issued")
            .map(|e| {
                (
                    e.payload["capability"]["cap"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    e.payload["capability"]["scope"].clone(),
                )
            })
            .collect()
    }

    pub fn tainted(&self) -> bool {
        self.broker
            .session_security(&Holder::agent("s1", "delta").session)
            .tainted
    }
}
