//! Lokalny serwer odpowiadający jak dostawca API (wynik zależy od klucza w nagłówku) i pomoce
//! testów kluczy: rdzeń na katalogu tymczasowym, dane konta, odczyt stanu.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use super::*;
use app_core::dto::{Account, AccountState, AddAccountInput, SecretInput};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths, MemorySecretStore};

pub const GOOD: &str = "test-dobry-XXXXXXXXXXXXXXXXXXXX";
pub const GOOD_2: &str = "test-drugi-XXXXXXXXXXXXXXXXXXXX";
pub const BAD: &str = "test-bad-XXXXXXXXXXXXXXXXXXXXXX";
pub const RESTRICTED: &str = "test-restricted-XXXXXXXXXXXXXXX";
pub const NO_CREDIT: &str = "test-nocredit-XXXXXXXXXXXXXXXXX";
pub const NO_BALANCE: &str = "test-payment-XXXXXXXXXXXXXXXXXX";
pub const REGION: &str = "test-region-XXXXXXXXXXXXXXXXXXX";
pub const NO_MODELS: &str = "test-nomodels-XXXXXXXXXXXXXXXXX";
pub const RATE_LIMITED: &str = "test-ratelimit-XXXXXXXXXXXXXXXX";

/// Odpowiedź serwera udającego dostawcę zgodnego z OpenAI — zależy od klucza w `Authorization`.
pub fn reply(key: &str) -> (&'static str, String) {
    let error = |message: &str, kind: &str, code: Option<&str>| {
        let code = code.map_or_else(String::new, |c| format!(r#","code":"{c}""#));
        format!(r#"{{"error":{{"message":"{message}","type":"{kind}"{code}}}}}"#)
    };
    if key.contains("restricted") {
        let body = error(
            "You have insufficient permissions for this operation. Missing scopes: api.model.read.",
            "invalid_request_error",
            None,
        );
        ("401 Unauthorized", body)
    } else if key.contains("nocredit") {
        let body = error(
            "You exceeded your current quota, please check your plan and billing details.",
            "insufficient_quota",
            Some("insufficient_quota"),
        );
        ("429 Too Many Requests", body)
    } else if key.contains("payment") {
        (
            "402 Payment Required",
            error("Insufficient Balance", "error", None),
        )
    } else if key.contains("region") {
        let body = error(
            "Country, region, or territory not supported",
            "request_forbidden",
            Some("unsupported_country_region_territory"),
        );
        ("403 Forbidden", body)
    } else if key.contains("ratelimit") {
        let body = error(
            "Rate limit reached",
            "requests",
            Some("rate_limit_exceeded"),
        );
        ("429 Too Many Requests", body)
    } else if key.contains("nomodels") {
        ("404 Not Found", error("Not Found", "not_found", None))
    } else if key.contains("bad") {
        // Tak odpowiada OpenAI: klucz zamaskowany, z prawdziwym początkiem i końcem.
        let body = error(
            "Incorrect API key provided: sk-proj-************************wxyz. You can find your API key at the provider page.",
            "invalid_request_error",
            Some("invalid_api_key"),
        );
        ("401 Unauthorized", body)
    } else {
        let body = r#"{"object":"list","data":[{"id":"model-testowy","object":"model"}]}"#;
        ("200 OK", body.to_owned())
    }
}

/// Uruchamia serwer; zwraca adres bazowy i listę kluczy, które do niego dotarły.
pub fn provider_stub() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 1024];
            while !buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buffer.extend_from_slice(&chunk[..n]),
                }
            }
            let request = String::from_utf8_lossy(&buffer).to_string();
            let key = request
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("authorization")
                        .then(|| value.trim().trim_start_matches("Bearer ").to_owned())
                })
                .unwrap_or_default();
            log.lock().unwrap().push(key.clone());
            let (status, body) = reply(&key);
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (base, seen)
}

pub fn input(secret: &str, base: &str, label: &str) -> AddAccountInput {
    serde_json::from_value(serde_json::json!({
        "provider_id": "custom-openai-compatible",
        "label": label,
        "secret": secret,
        "base_url": base,
    }))
    .unwrap()
}

pub fn secret(value: &str) -> SecretInput {
    serde_json::from_value(serde_json::json!(value)).unwrap()
}

pub async fn core() -> (AppCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.secrets = Some(Arc::new(MemorySecretStore::default()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    (core, dir)
}

pub async fn listed(core: &AppCore, id: &str) -> Account {
    let accounts = core.accounts_list().await.unwrap();
    accounts.into_iter().find(|a| a.id == id).unwrap()
}

/// Stan konta i powód ostatniego testu z listy kont.
pub async fn state_of(core: &AppCore, id: &str) -> (AccountState, String) {
    let account = listed(core, id).await;
    (account.state, account.last_test.unwrap().reason)
}
