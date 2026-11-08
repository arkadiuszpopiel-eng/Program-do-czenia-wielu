//! Serwer MCP Alfy **na żądanie mostu**: kanał lokalny (named pipe z ACL / gniazdo 0600, bez
//! TCP) otwiera się dopiero przy pierwszej rejestracji zadania mostu, nie przy starcie aplikacji.
//! v0: schowek, okna, `approve`; v1 (F6, [`mcp_v1`]): UI Automation i zrzuty przez **te same
//! narzędzia agentek** (Broker `gui.control`, strażnik okien Alfy, maskowanie) oraz rejestr tylko
//! do odczytu — podmiot Brokera „most CLI”, wyniki `unverified_by_alfa`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use mcp_contract::{
    ApprovalRouter, BridgeMcpHost, BridgeRegistration, BridgeScope, McpError, RegistrationId,
};
use mcp_impl::{HostConfig, LocalMcpHost, McpV1, MonotonicClock, PlatformPorts};
use safety_broker_contract::Broker;
use tools_common_contract::Tool;

/// Zależności serwera v1: narzędzia GUI agentek (z nich serwer bierze wyłącznie `uia_tree`,
/// `uia_find`, `uia_read_text`, `uia_act`, `screen_capture` z `gui.control`), rejestr
/// `WinRegistry` (poza Windows odpowiada „nieobsługiwane” — bez skutków), Broker (zwykle rejestr
/// kart nad Brokerem), magistrala (`mcp.alfa.call`) i limit czekania na zgodę w Broker-UI.
pub fn mcp_v1(
    gui_tools: Vec<Arc<dyn Tool>>,
    broker: Arc<dyn Broker>,
    bus: Option<Arc<dyn EventBus>>,
    approval_timeout: Duration,
) -> McpV1 {
    McpV1 {
        gui_tools,
        registry: Arc::new(platform_windows_office_impl::WinRegistry::new()),
        broker,
        bus,
        approval_timeout,
    }
}

/// Host MCP uruchamiany leniwie.
pub struct LazyMcpHost {
    config: HostConfig,
    ports: PlatformPorts,
    v1: Option<McpV1>,
    host: Mutex<Option<Arc<LocalMcpHost>>>,
}

impl LazyMcpHost {
    /// Host z programem proxy (`alfa-mcp-proxy` obok pliku wykonywalnego Alfy).
    pub fn new(proxy: PathBuf, ports: PlatformPorts) -> Self {
        Self {
            config: HostConfig::new(proxy),
            ports,
            v1: None,
            host: Mutex::new(None),
        }
    }

    /// Z narzędziami v1 (`None` — tylko v0, np. bez Brokera agentek).
    pub fn with_v1(mut self, v1: Option<McpV1>) -> Self {
        self.v1 = v1;
        self
    }

    /// Czy serwer wystawi narzędzia v1.
    pub fn has_v1(&self) -> bool {
        self.v1.is_some()
    }

    /// Czy kanał jest otwarty (diagnostyka, karta mostu).
    pub fn running(&self) -> bool {
        self.host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    fn get(&self) -> Result<Arc<LocalMcpHost>, McpError> {
        let mut guard = self.host.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(h) = guard.as_ref() {
            return Ok(h.clone());
        }
        let host = Arc::new(LocalMcpHost::start_with(
            self.config.clone(),
            self.ports.clone(),
            self.v1.clone(),
            Arc::new(MonotonicClock::default()),
        )?);
        *guard = Some(host.clone());
        Ok(host)
    }
}

/// Domyślna ścieżka `alfa-mcp-proxy` (obok pliku wykonywalnego aplikacji).
pub fn proxy_program() -> PathBuf {
    let name = if cfg!(windows) {
        "alfa-mcp-proxy.exe"
    } else {
        "alfa-mcp-proxy"
    };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

#[async_trait]
impl BridgeMcpHost for LazyMcpHost {
    async fn register(
        &self,
        scope: BridgeScope,
        approvals: Option<Arc<dyn ApprovalRouter>>,
    ) -> Result<BridgeRegistration, McpError> {
        self.get()?.register(scope, approvals).await
    }

    async fn revoke(&self, id: &RegistrationId) -> Result<(), McpError> {
        let host = self
            .host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match host {
            Some(h) => h.revoke(id).await,
            None => Ok(()),
        }
    }
}
