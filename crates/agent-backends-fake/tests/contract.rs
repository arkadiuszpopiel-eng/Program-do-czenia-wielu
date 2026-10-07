//! Współdzielone testy kontraktowe `AgentBackend`: na atrapie w pamięci i na prawdziwych mostach
//! (Claude Code, Codex) z fałszywym CLI i atrapą hosta MCP.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;

use agent_backends_contract::BridgeKind;
use agent_backends_contract::contract_tests::{Env, run_all};
use agent_backends_fake::FakeAgentBackend;

#[tokio::test(flavor = "multi_thread")]
async fn fake_backend_passes_contract() {
    for bridge in BridgeKind::ALL {
        let env = Env {
            bridge,
            source: PathBuf::from("/zrodlo"),
        };
        run_all(env, |sink| async move { FakeAgentBackend::new(sink) }).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_bridge_passes_contract() {
    let source = common::git_repo(&common::temp_dir("src-claude"));
    let env = Env {
        bridge: BridgeKind::ClaudeCode,
        source,
    };
    run_all(env, |sink| async move {
        common::harness(sink, |_| {}).await.backend
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_bridge_passes_contract() {
    let source = common::git_repo(&common::temp_dir("src-codex"));
    let env = Env {
        bridge: BridgeKind::Codex,
        source,
    };
    run_all(env, |sink| async move {
        common::harness(sink, |_| {}).await.backend
    })
    .await;
}
