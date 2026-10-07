//! Wspólne pomocniki testów `tools-fs-impl`: wirtualny FS, Broker z prawdziwym silnikiem
//! (profil `/Users/ala`), dziennik cofania z rdzenia kontraktu w pamięci.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use compliance_contract::{DenyLists, PathEnv};
use core_bus_fake::FakeBus;
use platform_fake::FakeFs;
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::FakeBroker;
use tools_common_contract::{Tool, ToolCtx, ToolOutcome, Toolset};
use tools_fs_contract::{FsToolKind, FsToolsConfig};
use tools_fs_impl::{FsTools, FsToolsDeps};
use undo_journal_contract::{Journal, MemStore, UndoLimits};
use watchdog_contract::ManualClock;

pub const HOME: &str = "/Users/ala";

pub struct Harness {
    pub fs: Arc<FakeFs>,
    pub broker: Arc<FakeBroker>,
    pub journal: Arc<Journal>,
    pub bus: Arc<FakeBus>,
    pub tools: FsTools,
}

pub fn env() -> PathEnv {
    PathEnv::windows_profile(HOME)
}

pub fn harness(files: &[(&str, &str)]) -> Harness {
    let fs = Arc::new(FakeFs::with_files(
        files
            .iter()
            .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec())),
    ));
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker =
        Arc::new(FakeBroker::with(policy, env(), Arc::new(ManualClock::new(1_000_000))).unwrap());
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
    let bus = Arc::new(FakeBus::default());
    let tools = FsTools::new(FsToolsDeps {
        fs: fs.clone(),
        journal: journal.clone(),
        broker: broker.clone(),
        env: env(),
        deny: DenyLists::baseline(),
        config: FsToolsConfig {
            read_max_bytes: 64,
            ..FsToolsConfig::default()
        },
        bus: Some(bus.clone()),
    });
    Harness {
        fs,
        broker,
        journal,
        bus,
        tools,
    }
}

pub fn ctx() -> ToolCtx {
    ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(&format!("{HOME}/Documents"))
}

impl Harness {
    pub fn tool(&self, kind: FsToolKind) -> Arc<dyn Tool> {
        self.tools.tool(kind)
    }

    pub async fn call(&self, kind: FsToolKind, args: serde_json::Value) -> ToolOutcome {
        self.tool(kind).call(args, &ctx()).await
    }

    pub async fn call_ctx(
        &self,
        kind: FsToolKind,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> ToolOutcome {
        self.tool(kind).call(args, ctx).await
    }

    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.tools()
    }

    pub fn file(&self, p: &str) -> Option<String> {
        self.fs
            .snapshot()
            .get(&PathBuf::from(p))
            .map(|d| String::from_utf8_lossy(d).into_owned())
    }
}
