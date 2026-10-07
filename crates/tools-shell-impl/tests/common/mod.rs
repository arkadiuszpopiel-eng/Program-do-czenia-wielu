//! Pomocniki testów `tools-shell-impl`: wirtualny FS (snapshot zakresu), atrapa `ExecPort`,
//! Broker z prawdziwym silnikiem (profil `/Users/ala`), tabela Job Objects kill-switcha.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use core_bus_fake::FakeBus;
use platform_fake::{FakeExec, FakeFs};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::FakeBroker;
use tools_common_contract::{Tool, ToolCtx, ToolOutcome, Toolset};
use tools_shell_contract::ShellToolsConfig;
use tools_shell_impl::{ShellTools, ShellToolsDeps};
use undo_journal_contract::{Journal, MemStore, UndoLimits};
use watchdog_contract::{JobTable, ManualClock};

pub const HOME: &str = "/Users/ala";
pub const WORK: &str = "/Users/ala/Projekt";

pub struct Harness {
    pub fs: Arc<FakeFs>,
    pub exec: Arc<FakeExec>,
    pub broker: Arc<FakeBroker>,
    pub journal: Arc<Journal>,
    pub jobs: Arc<JobTable>,
    pub bus: Arc<FakeBus>,
    pub tools: ShellTools,
}

pub fn harness(files: &[(&str, &str)]) -> Harness {
    let env = PathEnv::windows_profile(HOME);
    let fs = Arc::new(FakeFs::with_files(
        files
            .iter()
            .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec())),
    ));
    let exec = Arc::new(FakeExec::new());
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
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
    let jobs = Arc::new(JobTable::default());
    let bus = Arc::new(FakeBus::default());
    let tools = ShellTools::new(ShellToolsDeps {
        exec: exec.clone(),
        journal: journal.clone(),
        broker: broker.clone(),
        env,
        deny: DenyLists::baseline(),
        config: ShellToolsConfig::default(),
        base_env: Some(vec![
            ("PATH".into(), "C:\\Windows".into()),
            ("ANTHROPIC_API_KEY".into(), "sk-ant-sekret".into()),
            ("GITHUB_TOKEN".into(), "ghp_x".into()),
            ("TEMP".into(), "C:\\t".into()),
            ("RANDOM".into(), "1".into()),
        ]),
        jobs: Some(jobs.clone()),
        bus: Some(bus.clone()),
    });
    Harness {
        fs,
        exec,
        broker,
        journal,
        jobs,
        bus,
        tools,
    }
}

pub fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(WORK);
    c.approval_timeout = Duration::from_millis(300);
    c
}

impl Harness {
    pub fn run(&self) -> Arc<dyn Tool> {
        self.tools.run_tool()
    }

    pub async fn sh(&self, command: &str) -> ToolOutcome {
        self.run()
            .call(serde_json::json!({ "command": command }), &ctx())
            .await
    }

    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.tools()
    }
}
