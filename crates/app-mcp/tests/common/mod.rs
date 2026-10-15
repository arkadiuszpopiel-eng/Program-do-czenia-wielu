//! Wspólne przygotowanie testów `app-mcp`: menedżer w katalogu tymczasowym, serwery w procesie
//! (`mcp-fake`), sekrety w pamięci.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use accounts_hub_contract::{CliOutput, CliProbe, SecretStore};
use accounts_hub_fake::MemorySecretStore;
use app_api::dto::{McpEnvVar, McpServerInput, McpTrust};
use app_mcp::{Connector, McpHub, McpHubDeps};
use async_trait::async_trait;
use mcp_contract::{McpClient, McpError, McpServerConfig, PinStore};
use mcp_fake::FakeMcpServer;
use mcp_impl::StdioMcpClient;

/// Serwery w procesie po identyfikatorze; zapamiętuje ostatnią konfigurację (środowisko).
#[derive(Default)]
pub struct InProc {
    pub servers: Mutex<BTreeMap<String, FakeMcpServer>>,
    pub last: Mutex<Option<McpServerConfig>>,
    pub connects: Mutex<u32>,
}

#[async_trait]
impl Connector for InProc {
    async fn connect(
        &self,
        config: McpServerConfig,
        pins: Arc<dyn PinStore>,
    ) -> Result<Arc<dyn McpClient>, McpError> {
        *self.connects.lock().unwrap() += 1;
        *self.last.lock().unwrap() = Some(config.clone());
        let server = self
            .servers
            .lock()
            .unwrap()
            .get(&config.id)
            .cloned()
            .ok_or_else(|| McpError::Spawn("brak atrapy".into()))?;
        let (r, w) = server.serve_duplex();
        Ok(Arc::new(StdioMcpClient::connect(config, r, w, pins).await?))
    }
}

/// Wyszukiwanie: każdy program „istnieje” poza `brak-programu`.
pub struct AnyProbe;

impl CliProbe for AnyProbe {
    fn locate(&self, program: &str) -> Option<PathBuf> {
        (program != "brak-programu").then(|| PathBuf::from(format!(r"C:\bin\{program}.exe")))
    }
    fn version_output(&self, _path: &Path) -> Option<String> {
        None
    }
    fn run_output(&self, _path: &Path, _args: &[&str]) -> Option<CliOutput> {
        None
    }
}

/// Katalog tymczasowy testu.
pub fn temp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("alfa-app-mcp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub struct Setup {
    pub hub: Arc<McpHub>,
    pub inproc: Arc<InProc>,
    pub secrets: Arc<MemorySecretStore>,
    pub dir: PathBuf,
}

pub fn hub_with(dir: &Path, inproc: Arc<InProc>, secrets: Arc<MemorySecretStore>) -> Arc<McpHub> {
    Arc::new(McpHub::new(McpHubDeps {
        config_dir: dir.join("config"),
        work_dir: dir.join("work"),
        secrets: Some(secrets as Arc<dyn SecretStore>),
        probe: Some(Arc::new(AnyProbe)),
        connector: inproc,
    }))
}

pub fn setup(tag: &str) -> Setup {
    let dir = temp(tag);
    let inproc = Arc::new(InProc::default());
    let secrets = Arc::new(MemorySecretStore::new());
    let hub = hub_with(&dir, inproc.clone(), secrets.clone());
    Setup {
        hub,
        inproc,
        secrets,
        dir,
    }
}

/// Formularz serwera.
pub fn input(name: &str) -> McpServerInput {
    McpServerInput {
        id: None,
        name: name.into(),
        command: "npx".into(),
        args: vec!["-y".into(), "serwer".into()],
        cwd: None,
        env: vec![McpEnvVar {
            name: "LOG_LEVEL".into(),
            value: Some("info".into()),
            secret: false,
        }],
        trust: McpTrust::Untrusted,
        enabled: true,
        timeout_ms: Some(5_000),
    }
}
