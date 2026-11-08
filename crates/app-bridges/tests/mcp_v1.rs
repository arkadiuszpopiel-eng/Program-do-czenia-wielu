//! Serwer MCP Alfy v1 w aplikacji: z narzędzi agentek trafiają do mostu wyłącznie UIA i zrzut
//! (z `gui.control`) oraz rejestr tylko do odczytu; wejście syntetyczne i inne narzędzia — nigdy.
//! Rejestracja zakresu v1 z sesją rozmowy otwiera kanał leniwie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use app_bridges::{LazyMcpHost, mcp_v1};
use mcp_contract::{AlfaTool, BridgeMcpHost, BridgeScope};
use mcp_impl::{PlatformPorts, V1Tools};
use platform_fake::{FakeClipboard, FakeWindows};
use safety_broker_fake::FakeBroker;
use tools_common_contract::{Tool, Toolset};

fn gui_tools() -> Vec<Arc<dyn Tool>> {
    let mut tools = tools_uia_fake::FakeTools::default().tools();
    tools.extend(tools_screen_fake::FakeTools::default().tools());
    tools.extend(tools_input_fake::FakeTools::default().tools());
    tools
}

#[test]
fn v1_exposes_only_uia_screen_and_registry() {
    let broker = Arc::new(FakeBroker::new().unwrap());
    let v1 = V1Tools::new(mcp_v1(gui_tools(), broker, None, Duration::from_secs(1)));
    for t in AlfaTool::WINDOWS_V1 {
        if t.is_v1() {
            assert!(v1.available(t), "{t:?}");
        }
    }
    let without = V1Tools::new(mcp_v1(
        Vec::new(),
        Arc::new(FakeBroker::new().unwrap()),
        None,
        Duration::from_secs(1),
    ));
    assert!(without.available(AlfaTool::RegistryRead), "rejestr zawsze");
    assert!(!without.available(AlfaTool::UiaTree));
}

#[tokio::test]
async fn lazy_host_with_v1_opens_on_first_bridge_registration() {
    let ports = PlatformPorts {
        clipboard: Arc::new(FakeClipboard::default()),
        windows: Arc::new(FakeWindows::default()),
    };
    let broker = Arc::new(FakeBroker::new().unwrap());
    let host = LazyMcpHost::new("alfa-mcp-proxy".into(), ports).with_v1(Some(mcp_v1(
        gui_tools(),
        broker,
        None,
        Duration::from_secs(1),
    )));
    assert!(host.has_v1() && !host.running());
    let reg = host
        .register(
            BridgeScope::windows_v1("task-1").with_session("czat-1"),
            None,
        )
        .await
        .unwrap();
    assert!(host.running());
    host.revoke(&reg.id).await.unwrap();
}
