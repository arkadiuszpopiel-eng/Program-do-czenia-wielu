//! Serwery MCP w rdzeniu (`mcp_*`): zapis bez sekretów w pliku, stan po nieudanym uruchomieniu
//! z czytelnym powodem, usuwanie; brak programu w PATH — podpowiedź zamiast awarii.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use accounts_hub_contract::{CliOutput, CliProbe};
use app_core::dto::{McpConnState, McpEnvVar, McpServerInput, McpTrust};
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, options};

/// `npx` „jest” w PATH, ale plik nie istnieje (uruchomienie się nie uda); reszty nie ma.
struct Probe;

impl CliProbe for Probe {
    fn locate(&self, program: &str) -> Option<PathBuf> {
        (program == "npx").then(|| PathBuf::from(r"C:\nie\istnieje\npx.cmd"))
    }
    fn version_output(&self, _path: &Path) -> Option<String> {
        None
    }
    fn run_output(&self, _path: &Path, _args: &[&str]) -> Option<CliOutput> {
        None
    }
}

async fn core() -> (AppCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let mut opts = options(Some(provider), shell);
    opts.cli_probe = Some(Arc::new(Probe));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    (core, dir)
}

fn input(name: &str, command: &str) -> McpServerInput {
    McpServerInput {
        id: None,
        name: name.into(),
        command: command.into(),
        args: vec!["-y".into(), "@playwright/mcp@latest".into()],
        cwd: None,
        env: vec![McpEnvVar {
            name: "SERVICE_TOKEN".into(),
            value: Some("wartosc-tajna-42".into()),
            secret: true,
        }],
        trust: McpTrust::Untrusted,
        enabled: true,
        timeout_ms: Some(5_000),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn save_check_and_remove_through_core() {
    let (core, dir) = core().await;
    assert!(core.mcp_list().await.unwrap().is_empty());
    let saved = core.mcp_save(input("Przeglądarka", "npx")).await.unwrap();
    assert_eq!(saved.id, "przegladarka");
    assert_eq!(saved.state, McpConnState::Idle);
    assert!(saved.resolved.is_some());
    assert_eq!(saved.env[0].value, None);
    // Plik konfiguracji bez wartości tajnej.
    let files: Vec<_> = walk(dir.path());
    let config = files
        .iter()
        .find(|p| p.ends_with("mcp-servers.json"))
        .expect("plik serwerów MCP");
    let text = std::fs::read_to_string(config).unwrap();
    assert!(text.contains("SERVICE_TOKEN") && !text.contains("wartosc-tajna-42"));
    // Uruchomienie nieudane (program nie istnieje) — stan „błąd” z powodem, bez awarii.
    let tools = core.mcp_tools("przegladarka".into()).await.unwrap();
    assert_eq!(tools.server.state, McpConnState::Failed);
    let error = tools.server.error.unwrap();
    assert!(error.contains("uruchomić"), "{error}");
    // Brak programu w PATH — podpowiedź.
    let missing = core.mcp_save(input("Fetch", "uvx")).await.unwrap();
    assert!(missing.resolved.is_none());
    let t = core.mcp_tools(missing.id.clone()).await.unwrap();
    assert!(
        t.server
            .error
            .unwrap()
            .contains("Nie znaleziono programu `uvx`")
    );
    // Zgoda na nieistniejący odcisk — błąd, nie panika.
    assert!(
        core.mcp_approve("przegladarka".into(), "x".into(), "zly".into())
            .await
            .is_err()
    );
    core.mcp_remove("przegladarka".into()).await.unwrap();
    core.mcp_remove(missing.id).await.unwrap();
    assert!(core.mcp_list().await.unwrap().is_empty());
    assert!(core.mcp_remove("przegladarka".into()).await.is_err());
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}
