//! Przegląd bezpieczeństwa #3 (2026-10, SR3-05): narzędzie `clipboard_read` serwera MCP Alfy
//! (w zakresie każdego zadania mostu — `BridgeScope::windows_v1`) oddaje modelowi CLI zawartość
//! schowka **bez** redakcji sekretów i bez filtra ścieżek poświadczeń, w przeciwieństwie do
//! narzędzia schowka agentek (`tools-clipboard`: `redact_secrets`, deny-lista ścieżek). Wynik
//! trafia do dostawcy chmurowego mostu (S25, S28).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mcp_contract::{AlfaTool, ToolHandler};
use mcp_impl::{AlfaToolHandler, PlatformPorts};
use platform_contract::{ClipboardContent, ClipboardPort};
use platform_fake::{FakeClipboard, FakeWindows};
use serde_json::json;

fn handler(clipboard: Arc<FakeClipboard>) -> AlfaToolHandler {
    AlfaToolHandler::new(
        &[AlfaTool::ClipboardRead].into_iter().collect(),
        PlatformPorts {
            clipboard,
            windows: Arc::new(FakeWindows::default()),
        },
        None,
    )
}

#[tokio::test]
async fn bridge_clipboard_read_redacts_secrets() {
    let clipboard = Arc::new(FakeClipboard::new());
    let key = "sk-ant-api03-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    clipboard
        .set(ClipboardContent::Text(format!(
            "konfiguracja:\nANTHROPIC_API_KEY={key}\nhasło: Tajne123!"
        )))
        .unwrap();
    let out = handler(clipboard)
        .call("clipboard_read", json!({}))
        .await
        .unwrap();
    let text = serde_json::to_string(&out).unwrap();
    assert!(!text.contains(key), "klucz API trafił do mostu: {text}");
    assert!(
        !text.contains("Tajne123!"),
        "hasło trafiło do mostu: {text}"
    );
    assert!(
        text.contains("konfiguracja"),
        "zwykły tekst zostaje: {text}"
    );
}

#[tokio::test]
async fn bridge_clipboard_read_hides_credential_paths() {
    let clipboard = Arc::new(FakeClipboard::new());
    clipboard
        .set(ClipboardContent::Files(vec![
            r"C:\Users\ala\.ssh\id_ed25519".into(),
            r"C:\Users\ala\AppData\Roaming\Microsoft\Credentials\DFBE70A7".into(),
            r"C:\Users\ala\Dokumenty\raport.docx".into(),
        ]))
        .unwrap();
    let out = handler(clipboard)
        .call("clipboard_read", json!({}))
        .await
        .unwrap();
    let text = serde_json::to_string(&out).unwrap();
    assert!(!text.contains("id_ed25519"), "{text}");
    assert!(!text.contains("DFBE70A7"), "{text}");
    assert!(text.contains("raport.docx"), "{text}");
}
