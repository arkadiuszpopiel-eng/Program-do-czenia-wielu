//! Klient MCP nad prawdziwym procesem (`alfa-fake-mcp-server`): czyste środowisko (lista
//! dozwolona + jawne zmienne), sekret przekazany bez ujawnienia, zgoda i podmiana opisu między
//! uruchomieniami, zakończenie procesu przy `shutdown`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use mcp_contract::{
    ConsentOrigin, McpClient, McpServerConfig, MemoryPinStore, PinStore, TrustLevel,
};
use mcp_impl::StdioMcpClient;
use serde_json::json;

fn config(env: &[(&str, &str)]) -> McpServerConfig {
    McpServerConfig {
        id: "atrapa".into(),
        command: PathBuf::from(env!("CARGO_BIN_EXE_alfa-fake-mcp-server")),
        args: vec![],
        env: env
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        cwd: Some(std::env::temp_dir()),
        trust: TrustLevel::Untrusted,
        request_timeout_ms: 10_000,
    }
}

fn text(r: &mcp_contract::CallToolResult) -> String {
    match r.content.first() {
        Some(mcp_contract::Content::Text { text }) => text.clone(),
        other => panic!("brak tekstu: {other:?}"),
    }
}

async fn approve_all(client: &StdioMcpClient) {
    for t in client.tools().await.unwrap() {
        client
            .consent_tool(&t.tool.name, &t.fingerprint, ConsentOrigin::User)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn real_process_clean_env_secret_and_shutdown() {
    // Zmienne procesu Alfy (tu: ustawione przez cargo) nie mogą przejść do serwera.
    assert!(std::env::var_os("CARGO_MANIFEST_DIR").is_some());
    let pins: Arc<dyn PinStore> = Arc::new(MemoryPinStore::new());
    let client = StdioMcpClient::spawn(
        config(&[("ALFA_FAKE_SECRET", "abcdefgh"), ("JAWNA", "1")]),
        pins.clone(),
    )
    .await
    .unwrap();
    assert_eq!(client.server_info().unwrap().name, "alfa-fake-mcp-server");
    assert_eq!(client.tools().await.unwrap().len(), 3);
    approve_all(&client).await;
    let names = text(&client.call_tool("env_names", json!({})).await.unwrap());
    assert!(!names.contains("CARGO"), "{names}");
    assert!(names.contains("JAWNA") && names.contains("PATH"), "{names}");
    let len = text(&client.call_tool("secret_len", json!({})).await.unwrap());
    assert_eq!(len, "8");
    let echo = text(
        &client
            .call_tool("echo", json!({"text": "zażółć"}))
            .await
            .unwrap(),
    );
    assert_eq!(echo, "zażółć");
    client.shutdown().await;
    assert!(
        client
            .call_tool("echo", json!({"text": "x"}))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn changed_description_between_runs_is_blocked() {
    let pins: Arc<dyn PinStore> = Arc::new(MemoryPinStore::new());
    let first = StdioMcpClient::spawn(config(&[]), pins.clone())
        .await
        .unwrap();
    approve_all(&first).await;
    assert!(first.call_tool("echo", json!({"text": "a"})).await.is_ok());
    first.shutdown().await;
    let second = StdioMcpClient::spawn(
        config(&[("ALFA_FAKE_MCP_DESCRIPTION", "Nowy opis po zgodzie.")]),
        pins,
    )
    .await
    .unwrap();
    let err = second
        .call_tool("echo", json!({"text": "a"}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, mcp_contract::McpError::NeedsConsent { .. }),
        "{err}"
    );
    // Pozostałe narzędzia bez zmian działają dalej.
    assert!(second.call_tool("secret_len", json!({})).await.is_ok());
    second.shutdown().await;
}

#[tokio::test]
async fn missing_program_is_spawn_error() {
    let mut cfg = config(&[]);
    cfg.command = PathBuf::from(r"C:\nie\ma\serwera.exe");
    let pins: Arc<dyn PinStore> = Arc::new(MemoryPinStore::new());
    let err = StdioMcpClient::spawn(cfg, pins).await.err().unwrap();
    assert!(matches!(err, mcp_contract::McpError::Spawn(_)), "{err}");
}

/// Serwer przez plik wsadowy (`npx.cmd`, `uvx.cmd` na Windows): uruchomienie przez `cmd`, stdio
/// przechodzi bez zmian.
#[cfg(windows)]
#[tokio::test]
async fn batch_wrapper_like_npx_works() {
    let dir = std::env::temp_dir().join(format!("alfa-mcp-cmd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wrapper = dir.join("serwer-mcp.cmd");
    std::fs::write(
        &wrapper,
        format!(
            "@echo off\r\n\"{}\" %*\r\n",
            env!("CARGO_BIN_EXE_alfa-fake-mcp-server")
        ),
    )
    .unwrap();
    let mut cfg = config(&[]);
    cfg.command = wrapper;
    cfg.args = vec!["--argument z spacją".into()];
    let pins: Arc<dyn PinStore> = Arc::new(MemoryPinStore::new());
    let client = StdioMcpClient::spawn(cfg, pins).await.unwrap();
    approve_all(&client).await;
    let echo = text(
        &client
            .call_tool("echo", json!({"text": "ok"}))
            .await
            .unwrap(),
    );
    assert_eq!(echo, "ok");
    client.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn failed_start_reports_stderr_with_masked_values() {
    let pins: Arc<dyn PinStore> = Arc::new(MemoryPinStore::new());
    let err = StdioMcpClient::spawn(
        config(&[("ALFA_FAKE_MCP_FAIL", "pakiet-tajny-77 nie istnieje")]),
        pins,
    )
    .await
    .err()
    .unwrap();
    let text = err.to_string();
    assert!(text.contains("npm ERR! 404"), "{text}");
    assert!(
        !text.contains("pakiet-tajny-77"),
        "wartości zmiennych maskowane: {text}"
    );
}
