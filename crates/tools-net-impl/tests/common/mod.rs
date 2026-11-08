//! Harness testów `tools-net`: wirtualna sieć (`FakeHttp`), kwarantanna w pamięci
//! (`FakeDownloads`), Broker z prawdziwym silnikiem (skrypt „zezwól” albo prawdziwy klasyfikator
//! z egress-allowlistą), magistrala-atrapa; asercje „szpiegowskie” na dzienniku żądań i tokenach.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use core_bus_fake::FakeBus;
use platform_apps_fake::FakeDownloads;
use safety_broker_contract::{Broker, Holder, HostPattern, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use tools_common_contract::{Tool, ToolCtx, Toolset};
use tools_net_contract::{NetToolsConfig, SearchPort};
use tools_net_fake::FakeHttp;
use tools_net_impl::{NetTools, NetToolsDeps};
use watchdog_contract::ManualClock;

/// Ścieżka bezwzględna właściwa dla systemu (`C:\…` na Windows, `/…` gdzie indziej).
pub fn abs(rel: &str) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" });
    rel.split('/')
        .filter(|s| !s.is_empty())
        .fold(root, |acc, seg| acc.join(seg))
}

pub fn home() -> String {
    abs("Users/ala").to_string_lossy().into_owned()
}

pub fn workdir() -> String {
    abs("Users/ala/Alfa/Sesje/s1")
        .to_string_lossy()
        .into_owned()
}

pub struct H {
    pub net: Arc<FakeHttp>,
    pub downloads: FakeDownloads,
    pub broker: Arc<FakeBroker>,
    pub bus: Arc<FakeBus>,
    pub tools: NetTools,
}

/// Konfiguracja harnessu.
pub struct Opts {
    /// Skrypt „zezwól” dla wszystkich narzędzi (inaczej prawdziwy klasyfikator, L3).
    pub allow: bool,
    /// Egress-allowlista polityki Jądra.
    pub allowlist: Vec<&'static str>,
    /// Wyszukiwarka.
    pub search: Option<Arc<dyn SearchPort>>,
    /// Korzeń kwarantanny bez katalogu roboczego.
    pub root: Option<PathBuf>,
    /// Limity.
    pub config: NetToolsConfig,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            allow: true,
            allowlist: Vec::new(),
            search: None,
            root: None,
            config: NetToolsConfig::default(),
        }
    }
}

pub fn harness_with(o: Opts) -> H {
    let env = PathEnv::windows_profile(&home());
    let mut policy = KernelPolicy::baseline(&home(), "/ProgramData/AlfaBroker").unwrap();
    policy.egress_allowlist = o
        .allowlist
        .iter()
        .map(|h| HostPattern::parse(h).unwrap())
        .collect();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    if o.allow {
        for t in ["fetch", "download", "search"] {
            broker.script(&format!("tools-net.{t}"), ScriptedDecision::Allow);
        }
    }
    let net = Arc::new(FakeHttp::default());
    let downloads = FakeDownloads::default();
    let bus = Arc::new(FakeBus::default());
    let tools = NetTools::new(NetToolsDeps {
        http: net.clone(),
        search: o.search,
        downloads: Arc::new(downloads.clone()),
        quarantine_root: o.root,
        broker: broker.clone(),
        deny: DenyLists::baseline(),
        env,
        config: o.config,
        bus: Some(bus.clone()),
    });
    H {
        net,
        downloads,
        broker,
        bus,
        tools,
    }
}

pub fn harness() -> H {
    harness_with(Opts::default())
}

pub fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(&workdir());
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

    /// Hosty z tokenem `net.egress` (kolejność wydania).
    pub fn egress_tokens(&self) -> Vec<String> {
        self.tokens("net.egress")
    }

    /// Zakresy tokenów danej rodziny.
    pub fn tokens(&self, family: &str) -> Vec<String> {
        self.broker
            .audit_events()
            .iter()
            .filter(|e| {
                e.kind.as_str() == "broker.token.issued" && e.payload["capability"]["cap"] == family
            })
            .map(|e| match &e.payload["capability"]["scope"] {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect()
    }

    pub fn tainted(&self) -> bool {
        self.broker
            .session_security(&Holder::agent("s1", "delta").session)
            .tainted
    }
}
