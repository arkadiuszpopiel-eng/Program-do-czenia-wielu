//! Host MCP Alfy: testy kontraktowe, TTL tokenu, `alfa-mcp-proxy` od końca do końca, narzędzia
//! schowka i okien przez porty platformy (okna chronione).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use mcp_contract::contract_tests::{ProxySide, RecordingRouter, run_all};
use mcp_contract::{
    AlfaTool, BridgeMcpHost, BridgeScope, ConsentOrigin, McpClient, McpServerConfig,
    MemoryPinStore, ToolState, TrustLevel,
};
use mcp_impl::{Clock, HostConfig, LocalMcpHost, PlatformPorts, StdioMcpClient, listener};
use platform_contract::{ClipboardContent, ClipboardPort, WindowId, WindowInfo, WindowPort};
use platform_fake::{FakeClipboard, FakeWindows};
use serde_json::json;

#[derive(Default)]
struct ManualClock(AtomicU64);

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

fn window(id: u64, title: &str, process: &str) -> WindowInfo {
    WindowInfo {
        id: WindowId(id),
        title: title.into(),
        process: process.into(),
        focused: false,
        fullscreen: false,
    }
}

struct Fixture {
    host: LocalMcpHost,
    clock: Arc<ManualClock>,
    clipboard: Arc<FakeClipboard>,
    windows: Arc<FakeWindows>,
}

fn fixture(ttl_ms: u64) -> Fixture {
    let clipboard = Arc::new(FakeClipboard::new());
    let windows = Arc::new(FakeWindows::default());
    windows.add(window(1, "Notatnik", "notepad.exe"));
    windows.add(window(2, "Alfa", r"C:\Program Files\Alfa\alfa.exe"));
    windows.add(window(3, "Zatwierdzenie", "broker-ui.exe"));
    let clock = Arc::new(ManualClock::default());
    let mut config = HostConfig::new(PathBuf::from(env!("CARGO_BIN_EXE_alfa-mcp-proxy")));
    config.token_ttl_ms = ttl_ms;
    let host = LocalMcpHost::start(
        config,
        PlatformPorts {
            clipboard: clipboard.clone(),
            windows: windows.clone(),
        },
        clock.clone(),
    )
    .unwrap();
    Fixture {
        host,
        clock,
        clipboard,
        windows,
    }
}

#[tokio::test]
async fn host_passes_contract() {
    let f = fixture(60_000);
    run_all(&f.host, |ep| async move { listener::connect(&ep).await }).await;
    assert!(f.host.rejections().len() >= 2);
}

#[tokio::test]
async fn expired_and_garbage_hello_are_rejected() {
    let f = fixture(1_000);
    let reg = f
        .host
        .register(BridgeScope::windows_v0("t"), None)
        .await
        .unwrap();
    f.clock.0.store(999, Ordering::SeqCst);
    let ok = listener::connect(f.host.endpoint()).await.unwrap();
    let mut side = ProxySide::hello(ok, reg.launch.token().unwrap()).await;
    assert!(side.initialize().await.is_some(), "token przed TTL działa");
    f.clock.0.store(1_000, Ordering::SeqCst);
    let late = listener::connect(f.host.endpoint()).await.unwrap();
    let mut side = ProxySide::hello(late, reg.launch.token().unwrap()).await;
    assert!(side.initialize().await.is_none(), "wygasły token przyjęty");
    // Połączenie nawiązane przed TTL działa dalej do unieważnienia.
    let early = ProxySide::hello(listener::connect(f.host.endpoint()).await.unwrap(), "x").await;
    drop(early);
    assert!(f.host.rejections().iter().any(|r| r.contains("wygasł")));
}

fn proxy_config(
    launch: &mcp_contract::McpServerLaunch,
    id: &str,
    trust: TrustLevel,
) -> McpServerConfig {
    McpServerConfig {
        id: id.into(),
        command: launch.command.clone(),
        args: launch.args.clone(),
        env: launch.env.clone(),
        cwd: None,
        trust,
        request_timeout_ms: 10_000,
    }
}

/// Pełna ścieżka mostu: klient MCP (jak CLI) → proces `alfa-mcp-proxy` → kanał lokalny → host.
#[tokio::test]
async fn proxy_binary_end_to_end_with_platform_ports() {
    let f = fixture(60_000);
    let router = RecordingRouter::allowing();
    let reg = f
        .host
        .register(BridgeScope::windows_v0("e2e"), Some(router.clone()))
        .await
        .unwrap();
    let pins = Arc::new(MemoryPinStore::new());
    let client =
        StdioMcpClient::spawn(proxy_config(&reg.launch, "alfa", TrustLevel::Trusted), pins)
            .await
            .unwrap();
    assert_eq!(client.server_info().unwrap().name, "alfa");
    let tools = client.tools().await.unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t.tool.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "approve",
            "clipboard_read",
            "clipboard_write",
            "windows_focus",
            "windows_list"
        ]
    );
    assert!(
        tools
            .iter()
            .all(|t| t.state == ToolState::AutoApproved && !t.untrusted)
    );

    let listed = client.call_tool("windows_list", json!({})).await.unwrap();
    let ws = listed.structured_content.unwrap();
    assert_eq!(
        ws["windows"].as_array().unwrap().len(),
        1,
        "okna chronione ukryte: {ws}"
    );
    let focused = client
        .call_tool("windows_focus", json!({"id": 1}))
        .await
        .unwrap();
    assert!(!focused.is_error);
    assert!(
        f.windows
            .list()
            .iter()
            .any(|w| w.id == WindowId(1) && w.focused)
    );
    let protected = client.call_tool("windows_focus", json!({"id": 2})).await;
    assert!(
        matches!(
            protected,
            Err(mcp_contract::McpError::Rpc { code: -32001, .. })
        ),
        "{protected:?}"
    );

    client
        .call_tool("clipboard_write", json!({"text": "zażółć"}))
        .await
        .unwrap();
    assert_eq!(
        f.clipboard.get().unwrap(),
        ClipboardContent::Text("zażółć".into())
    );
    let read = client.call_tool("clipboard_read", json!({})).await.unwrap();
    assert_eq!(read.structured_content.unwrap()["text"], "zażółć");
    let bad = client.call_tool("clipboard_write", json!({})).await;
    assert!(bad.is_err());

    let approve = client
        .call_tool(
            "approve",
            json!({"tool_name": "Bash", "input": {"command": "ls"}}),
        )
        .await
        .unwrap();
    assert!(approve.joined_text().contains("\"behavior\":\"allow\""));
    assert_eq!(router.seen()[0].tool_name, "Bash");
    let consent = client
        .consent_tool(
            "approve",
            &tools[0].fingerprint,
            ConsentOrigin::Agent("alfa".into()),
        )
        .await;
    assert_eq!(consent, Err(mcp_contract::McpError::ConsentNotPermitted));

    // Unieważnienie zamyka połączenie nawiązane przez proxy.
    f.host.revoke(&reg.id).await.unwrap();
    let after = client.call_tool("windows_list", json!({})).await;
    assert!(
        after.is_err(),
        "po unieważnieniu wywołanie przeszło: {after:?}"
    );
    client.shutdown().await;
}

#[tokio::test]
async fn proxy_without_env_fails_cleanly() {
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_alfa-mcp-proxy"))
        .env_clear()
        .output()
        .await
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ALFA_MCP_ENDPOINT"), "{err}");
    let tcp = tokio::process::Command::new(env!("CARGO_BIN_EXE_alfa-mcp-proxy"))
        .env_clear()
        .envs(BTreeMap::from([
            ("ALFA_MCP_ENDPOINT", "tcp:localhost:1"),
            ("ALFA_MCP_TOKEN", "x"),
        ]))
        .output()
        .await
        .unwrap();
    assert!(!tcp.status.success(), "proxy przyjęło kanał TCP");
    assert!(!String::from_utf8_lossy(&tcp.stderr).contains("x\n"));
}

#[tokio::test]
async fn scope_without_windows_tools() {
    let f = fixture(60_000);
    let scope = BridgeScope {
        label: "tylko-schowek".into(),
        tools: [AlfaTool::ClipboardRead].into_iter().collect(),
        session: None,
    };
    let reg = f.host.register(scope, None).await.unwrap();
    let client = StdioMcpClient::spawn(
        proxy_config(&reg.launch, "alfa", TrustLevel::Trusted),
        Arc::new(MemoryPinStore::new()),
    )
    .await
    .unwrap();
    let tools = client.tools().await.unwrap();
    assert_eq!(tools.len(), 1);
    assert!(client.call_tool("windows_list", json!({})).await.is_err());
    client.shutdown().await;
}
