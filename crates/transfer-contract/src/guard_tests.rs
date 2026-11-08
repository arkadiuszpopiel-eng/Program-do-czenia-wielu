use super::*;

fn guard() -> SecretGuard {
    SecretGuard::new(vec![
        SecretString::from("moj-klucz-prywatny-123"),
        SecretString::from("krotki"),
    ])
}

#[test]
fn sensitive_keys() {
    for k in [
        "api_key",
        "apiKey",
        "API-KEY",
        "access_token",
        "password",
        "client.secret",
    ] {
        assert!(SecretGuard::is_sensitive_key(k), "{k}");
    }
    for k in [
        "max_tokens",
        "tokenizer",
        "keyboard",
        "title",
        "passwords_hint",
    ] {
        assert!(!SecretGuard::is_sensitive_key(k), "{k}");
    }
}

#[test]
fn text_redaction_counts_exact_and_patterns() {
    let g = guard();
    let (out, n) = g.clean_text("klucz moj-klucz-prywatny-123 i sk-ant-api03-abcdefghijk");
    assert_eq!(n, 2);
    assert!(!out.contains("moj-klucz") && !out.contains("sk-ant"));
    let (same, zero) = g.clean_text("zwykły tekst krotki");
    assert_eq!(zero, 0, "krótkie wartości nie są dopasowywane dokładnie");
    assert!(matches!(same, Cow::Borrowed(_)));
}

#[test]
fn documents_keep_syntax() {
    let g = guard();
    let toml_doc =
        "[providers]\napi_key = \"abc\"\nname = \"x moj-klucz-prywatny-123\"\nmax_tokens = 100\n";
    let (out, n) = g
        .clean_document("config/common/shared.toml", toml_doc.into())
        .unwrap();
    assert_eq!(n, 2);
    let parsed: toml::Table = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
    assert_eq!(parsed["providers"]["api_key"].as_str(), Some(REDACTED));
    assert_eq!(parsed["providers"]["max_tokens"].as_integer(), Some(100));
    assert!(!g.find_leak(&out));

    let json_doc = br#"{"tool":{"password":"hunter2","n":1},"text":"Bearer abcdefghijklmnop"}"#;
    let (out, n) = g
        .clean_document("personas/p.json", json_doc.to_vec())
        .unwrap();
    assert_eq!(n, 2);
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["tool"]["password"], serde_json::json!(REDACTED));
    assert_eq!(v["tool"]["n"], serde_json::json!(1));
    assert!(!g.find_leak(&out));

    let nd = "{\"a\":\"ok\"}\n{\"b\":\"xai-ABCDEFGH12345678\"}\n";
    let (out, n) = g.clean_document("memory/m.ndjson", nd.into()).unwrap();
    assert_eq!(n, 1);
    assert_eq!(std::str::from_utf8(&out).unwrap().lines().count(), 2);
    assert!(!g.find_leak(&out));

    let clean = b"{\"a\":1}".to_vec();
    assert_eq!(
        g.clean_document("x.json", clean.clone()).unwrap(),
        (clean, 0)
    );
}

#[test]
fn binary_and_leak_detection() {
    let g = guard();
    let mut bin = vec![0xff_u8, 0x00, 0xfe];
    bin.extend_from_slice(b"moj-klucz-prywatny-123");
    assert!(matches!(
        g.clean_document("artifacts/s/a.bin", bin.clone()),
        Err(TransferError::SecretDetected { .. })
    ));
    assert!(g.find_leak(&bin));
    assert!(!g.find_leak(&[
        0xff, 0xfe, b's', b'k', b'-', b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i'
    ]));
    assert!(g.find_leak(b"token = abcdefgh"));
    assert!(!g.find_leak(b"api_key = \"[REDACTED]\""));
    let quoted = SecretGuard::new(vec![SecretString::from("abc\"defgh\\ij")]);
    assert!(quoted.find_leak(br#"{"x":"abc\"defgh\\ij"}"#));
}
