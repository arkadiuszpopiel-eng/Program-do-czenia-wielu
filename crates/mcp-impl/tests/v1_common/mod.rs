//! Wspólne środowisko testów serwera MCP v1: rejestr z zasianymi sekretami, Broker z prawdziwym
//! silnikiem, atrapy narzędzi agentek UIA/zrzutów, handler z zakresem mostu.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use mcp_contract::BridgeScope;
use mcp_impl::{AlfaToolHandler, McpV1, PlatformPorts, V1Tools};
use platform_apps_contract::RegData;
use platform_apps_fake::FakeRegistry;
use platform_fake::{FakeClipboard, FakeWindows};
use safety_broker_contract::KernelPolicy;
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use tokio_util::sync::CancellationToken;
use tools_common_contract::{Tool, Toolset};
use watchdog_contract::ManualClock;

pub const SECRETS: [(&str, &str); 6] = [
    (r"HKCU\Software\Microsoft\Credentials\Cache", "Blob"),
    (r"HKLM\SECURITY\Policy\Secrets\DefaultPassword", "CurrVal"),
    (r"HKLM\SYSTEM\CurrentControlSet\Control\Lsa\JD", "Data"),
    (
        r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon",
        "DefaultPassword",
    ),
    (
        r"HKCU\Software\SimonTatham\PuTTY\Sessions\srv",
        "ProxyPassword",
    ),
    (r"HKCU\Software\ORL\WinVNC3", "Password"),
];

pub fn registry() -> Arc<FakeRegistry> {
    let r = Arc::new(FakeRegistry::new());
    for (i, (key, name)) in SECRETS.iter().enumerate() {
        r.set(key, name, RegData::String(format!("SEKRET-{i}")));
    }
    r.set(r"HKCU\Software\Acme", "Wersja", RegData::Dword(7));
    r.set(
        r"HKCU\Software\Acme",
        "ApiToken",
        RegData::String("SEKRET-TOKEN".into()),
    );
    r.set(
        r"HKCU\Software\Acme",
        "Opis",
        RegData::String("klucz=sk-ant-api03-ABCDEFGHIJKLMNOPQRSTUV".into()),
    );
    r.set(
        r"HKCU\Software\Acme\Ustawienia",
        "Kolor",
        RegData::String("zielony".into()),
    );
    r.set(
        r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "ProductName",
        RegData::String("Windows 11 Pro".into()),
    );
    r
}

pub fn broker(allow: bool) -> Arc<FakeBroker> {
    let policy = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    let env = PathEnv::windows_profile(r"C:\Users\ala");
    let b = Arc::new(FakeBroker::with(policy, env, Arc::new(ManualClock::new(1_000_000))).unwrap());
    if allow {
        b.script(mcp_impl::v1::REGISTRY_TOOL_ID, ScriptedDecision::Allow);
    }
    b
}

pub struct Env {
    pub uia: tools_uia_fake::FakeTools,
    pub screen: tools_screen_fake::FakeTools,
    pub registry: Arc<FakeRegistry>,
    pub broker: Arc<FakeBroker>,
    pub v1: Arc<V1Tools>,
}

pub fn env(allow: bool) -> Env {
    let uia = tools_uia_fake::FakeTools::default();
    let screen = tools_screen_fake::FakeTools::default();
    let (registry, broker) = (registry(), broker(allow));
    let mut gui: Vec<Arc<dyn Tool>> = uia.tools();
    gui.extend(screen.tools());
    let v1 = Arc::new(V1Tools::new(McpV1 {
        gui_tools: gui,
        registry: registry.clone(),
        broker: broker.clone(),
        bus: None,
        approval_timeout: Duration::from_millis(200),
    }));
    Env {
        uia,
        screen,
        registry,
        broker,
        v1,
    }
}

pub fn ports() -> PlatformPorts {
    PlatformPorts {
        clipboard: Arc::new(FakeClipboard::new()),
        windows: Arc::new(FakeWindows::default()),
    }
}

pub fn handler(e: &Env, scope: &BridgeScope) -> AlfaToolHandler {
    AlfaToolHandler::new(&scope.tools, ports(), None).with_v1(
        scope,
        Some(e.v1.clone()),
        CancellationToken::new(),
    )
}
