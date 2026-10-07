//! Wspólne przygotowanie testów mostów: repozytorium źródłowe, rejestr zgodności z repo,
//! atrapa hosta MCP, konfiguracja wskazująca fałszywe CLI.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use agent_backends_contract::{ApprovalSink, BridgeKind, CliPin};
use agent_backends_impl::{BackendDeps, BridgeBackend, BridgeConfig, GitWorkspace};
use compliance_contract::{ChangeOrigin, Compliance, Registry, RouteId};
use compliance_fake::FakeCompliance;
use mcp_fake::FakeBridgeMcpHost;

/// Wersja zgłaszana przez fałszywe CLI.
pub const FAKE_VERSION: &str = "9.8.7";

/// Ścieżka fałszywego CLI.
pub fn fake_cli() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_alfa-fake-agent-cli"))
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Usuwa katalogi poprzednich przebiegów (procesy, które już nie żyją) — oszczędza dysk.
fn remove_stale() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(pid) = name
            .strip_prefix("alfa-ab-")
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        let alive = cfg!(not(unix)) || Path::new(&format!("/proc/{pid}")).exists();
        if pid != std::process::id() && !alive {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Unikalny katalog tymczasowy (wspólny katalog procesu `alfa-ab-<pid>`).
pub fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    if n == 0 {
        remove_stale();
    }
    let dir = std::env::temp_dir()
        .join(format!("alfa-ab-{}", std::process::id()))
        .join(format!("{tag}-{n}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=alfa",
            "-c",
            "user.email=alfa@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
}

/// Repozytorium git z jednym plikiem.
pub fn git_repo(base: &Path) -> PathBuf {
    let src = base.join("zrodlo");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("README.txt"), "repo użytkownika").unwrap();
    git(&src, &["init", "-q"]);
    git(&src, &["add", "."]);
    git(&src, &["commit", "-q", "-m", "init"]);
    src
}

/// Atrapa zgodności z rejestrem z `docs/compliance` (Codex — trasa szara, włączona przez użytkownika).
pub async fn compliance() -> Arc<FakeCompliance> {
    let registry = Registry::from_json(include_str!(
        "../../../../docs/compliance/compliance-registry.json"
    ))
    .unwrap();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let c = Arc::new(FakeCompliance::new(registry, Vec::new(), today));
    let codex = RouteId::new("codex-cli").unwrap();
    c.set_enabled(&codex, true, ChangeOrigin::User)
        .await
        .unwrap();
    c
}

/// Konfiguracja z oboma mostami wskazującymi fałszywe CLI.
pub fn config(base: &Path) -> BridgeConfig {
    let pin = CliPin {
        versions: vec![FAKE_VERSION.into()],
        sha256: None,
    };
    let mut cfg = BridgeConfig::new(base.join("runtime"))
        .with_bridge(BridgeKind::ClaudeCode, fake_cli(), pin.clone())
        .with_bridge(BridgeKind::Codex, fake_cli(), pin);
    cfg.max_line_bytes = 1024 * 1024;
    cfg
}

/// Zestaw testowy.
pub struct Harness {
    /// Backend.
    pub backend: BridgeBackend,
    /// Host MCP.
    pub host: Arc<FakeBridgeMcpHost>,
    /// Zgodność.
    pub compliance: Arc<FakeCompliance>,
    /// Repozytorium źródłowe.
    pub source: PathBuf,
    /// Katalog worktree.
    pub worktrees: PathBuf,
}

/// Buduje zestaw; `tweak` zmienia konfigurację.
pub async fn harness(
    sink: Arc<dyn ApprovalSink>,
    tweak: impl FnOnce(&mut BridgeConfig),
) -> Harness {
    let base = temp_dir("h");
    let source = git_repo(&base);
    let worktrees = base.join("worktrees");
    let mut cfg = config(&base);
    tweak(&mut cfg);
    let host = Arc::new(FakeBridgeMcpHost::start(3_600_000).unwrap());
    let compliance = compliance().await;
    let backend = BridgeBackend::new(
        cfg,
        BackendDeps {
            compliance: compliance.clone(),
            workspace: Arc::new(GitWorkspace::new(&worktrees)),
            mcp: host.clone(),
            sink,
        },
    );
    Harness {
        backend,
        host,
        compliance,
        source,
        worktrees,
    }
}
