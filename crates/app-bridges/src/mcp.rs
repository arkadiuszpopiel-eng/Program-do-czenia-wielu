//! Serwer MCP Alfy v0 **na żądanie mostu**: kanał lokalny (named pipe z ACL / gniazdo 0600, bez
//! TCP) otwiera się dopiero przy pierwszej rejestracji zadania mostu, nie przy starcie aplikacji.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use mcp_contract::{
    ApprovalRouter, BridgeMcpHost, BridgeRegistration, BridgeScope, McpError, RegistrationId,
};
use mcp_impl::{HostConfig, LocalMcpHost, MonotonicClock, PlatformPorts};

/// Host MCP uruchamiany leniwie.
pub struct LazyMcpHost {
    config: HostConfig,
    ports: PlatformPorts,
    host: Mutex<Option<Arc<LocalMcpHost>>>,
}

impl LazyMcpHost {
    /// Host z programem proxy (`alfa-mcp-proxy` obok pliku wykonywalnego Alfy).
    pub fn new(proxy: PathBuf, ports: PlatformPorts) -> Self {
        Self {
            config: HostConfig::new(proxy),
            ports,
            host: Mutex::new(None),
        }
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
        let host = Arc::new(LocalMcpHost::start(
            self.config.clone(),
            self.ports.clone(),
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
