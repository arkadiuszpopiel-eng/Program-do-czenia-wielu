//! Serwer MCP Alfy v1 — rejestr: każdy odczyt przez Brokera (`system.admin(reg query …)`,
//! podmiot „most CLI”), bez zgody właściciela nic nie jest czytane, redakcja sekretów, deny-lista
//! kluczy — 0 odczytów sekretów w 200 losowych próbach.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod v1_common;

use mcp_contract::{BridgeScope, ToolCallError, ToolHandler};
use safety_broker_contract::Broker;
use safety_broker_fake::FakeBroker;
use serde_json::{Value, json};
use v1_common::{SECRETS, env, handler};

fn issued(b: &FakeBroker) -> Vec<Value> {
    b.audit_events()
        .into_iter()
        .filter(|e| e.kind.as_str() == "broker.token.issued")
        .map(|e| e.payload)
        .collect()
}

#[tokio::test]
async fn registry_goes_through_broker_and_redacts() {
    let e = env(true);
    let h = handler(&e, &BridgeScope::windows_v1("rej"));
    let out = h
        .call("registry_read", json!({"key": r"HKCU\Software\Acme"}))
        .await
        .unwrap();
    let text = serde_json::to_string(&out).unwrap();
    assert!(
        !text.contains("SEKRET") && !text.contains("sk-ant-api03"),
        "{text}"
    );
    let data = out.structured_content.unwrap();
    assert_eq!(data["unverified_by_alfa"], true);
    assert_eq!(data["subkeys"], json!(["Ustawienia"]));
    let one = h
        .call("registry_read", json!({"key": r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "value": "ProductName"}))
        .await
        .unwrap();
    assert_eq!(
        one.structured_content.unwrap()["data"]["value"],
        "Windows 11 Pro"
    );
    let tokens = issued(&e.broker);
    assert!(
        tokens
            .iter()
            .all(|t| t["tool"] == "mcp.registry_read" && t["capability"]["cap"] == "system.admin")
    );
    assert!(
        tokens
            .iter()
            .any(|t| t["capability"].to_string().contains("reg query"))
    );
    let denied = h
        .call("registry_read", json!({"key": r"HKLM\SAM\SAM"}))
        .await;
    assert!(matches!(denied, Err(ToolCallError::Unauthorized(_))));
    let hku = h
        .call("registry_read", json!({"key": r"HKU\S-1-5-18"}))
        .await;
    assert!(matches!(hku, Err(ToolCallError::InvalidParams(_))));
    assert!(e.broker.session_security(&"most:rej".into()).tainted);
}

#[tokio::test]
async fn registry_without_owner_consent_reads_nothing() {
    let e = env(false);
    let h = handler(&e, &BridgeScope::windows_v1("rej"));
    let out = h
        .call("registry_read", json!({"key": r"HKCU\Software\Acme"}))
        .await;
    assert!(
        matches!(out, Err(ToolCallError::Unauthorized(_))),
        "{out:?}"
    );
    assert_eq!(
        e.registry.reads(),
        0,
        "bez zgody właściciela rejestr nie jest czytany"
    );
}

/// Generator pseudolosowy (deterministyczny).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[usize::try_from(self.next() % items.len() as u64).unwrap()]
    }
}

fn mangle(rng: &mut Rng, key: &str) -> String {
    let (hive, rest) = key.split_once('\\').unwrap();
    let hive = match (hive, rng.next() % 4) {
        ("HKCU", 0) => "HKEY_CURRENT_USER",
        ("HKLM", 0) => "HKEY_LOCAL_MACHINE",
        ("HKCU", 1) => "Registry::HKEY_CURRENT_USER",
        ("HKLM", 1) => "hklm:",
        (h, _) => h,
    };
    let mut segs: Vec<String> = rest.split('\\').map(str::to_owned).collect();
    if rng.next().is_multiple_of(3) && segs.len() > 1 {
        segs.insert(1, "WOW6432Node".into());
    }
    let mut s = format!("{hive}\\{}", segs.join("\\"));
    s = s
        .chars()
        .map(|c| {
            if rng.next().is_multiple_of(2) {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    match rng.next() % 4 {
        0 => s.replace('\\', "/"),
        1 => s.replace('\\', "\\\\"),
        2 => format!("  {s}\\ "),
        _ => s,
    }
}

#[tokio::test]
async fn zero_secret_reads_in_200_random_trials() {
    let e = env(true);
    let h = handler(&e, &BridgeScope::windows_v1("losowe"));
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let benign = [
        r"HKCU\Software\Acme",
        r"HKCU\Software\Acme\Ustawienia",
        r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
    ];
    let values = [
        "",
        "ApiToken",
        "Password",
        "DefaultPassword",
        "Blob",
        "Data",
        "CurrVal",
        "Wersja",
        "Opis",
        "ProxyPassword",
    ];
    let mut leaks = 0;
    for _ in 0..200 {
        let base = if rng.next().is_multiple_of(3) {
            rng.pick(&benign)
        } else {
            SECRETS[usize::try_from(rng.next() % 6).unwrap()].0
        };
        let key = mangle(&mut rng, base);
        let args = if rng.next().is_multiple_of(2) {
            json!({"key": key})
        } else {
            json!({"key": key, "value": rng.pick(&values)})
        };
        let out = h.call("registry_read", args).await;
        let text = match &out {
            Ok(r) => serde_json::to_string(r).unwrap(),
            Err(err) => format!("{err:?}"),
        };
        if text.contains("SEKRET") {
            leaks += 1;
        }
    }
    assert_eq!(leaks, 0);
    assert_eq!(
        e.registry.raw_secret_reads(),
        0,
        "żaden klucz z sekretami nie dotarł do magazynu"
    );
    assert!(
        e.registry.reads() > 0,
        "odczyty dozwolonych kluczy działały"
    );
}
