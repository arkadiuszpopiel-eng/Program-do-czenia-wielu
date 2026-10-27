//! Filtr szczegółów błędów (`app_api::details`): dozwolone klucze i wartości, odrzucanie
//! sekretów i treści, limit wpisów, deserializacja przez ten sam filtr.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_api::details::*;

#[test]
fn accepts_ids_enums_numbers_and_flags() {
    let mut d = ErrorDetails::default();
    assert!(d.insert("kind", "session"));
    assert!(d.insert("id", "0198c1de-7a53-7c12-9b0e-3f2a6c1d4e5f"));
    assert!(d.insert("retry_after_s", 30u64));
    assert!(d.insert("retryable", true));
    assert_eq!(d.text("kind"), Some("session"));
    assert_eq!(d.number("retry_after_s"), Some(30));
    assert_eq!(d.get("retryable"), Some(&DetailValue::Flag(true)));
    assert_eq!(d.len(), 4);
}

#[test]
fn rejects_keys_named_like_secrets() {
    let mut d = ErrorDetails::default();
    for key in [
        "key",
        "api_key",
        "secret",
        "client_secret",
        "token",
        "access_token",
        "password",
        "authorization",
        "bearer",
        "cookie",
        "credential_id",
    ] {
        assert!(!d.insert(key, "x"), "klucz `{key}` powinien być odrzucony");
    }
    assert!(d.is_empty());
}

#[test]
fn rejects_malformed_keys() {
    let mut d = ErrorDetails::default();
    for key in ["", "Kind", "9lives", "with space", "żółć", "a-b"] {
        assert!(!d.insert(key, "x"), "klucz `{key}`");
    }
    assert!(!d.insert(&"a".repeat(MAX_KEY_LEN + 1), "x"));
}

#[test]
fn rejects_values_that_look_like_secrets_or_content() {
    let mut d = ErrorDetails::default();
    let long = "a".repeat(MAX_TEXT_LEN + 1);
    for value in [
        "sk-ant-api03-abc",
        "Bearer abc.def",
        "ghp_0123456789",
        "eyJhbGciOiJIUzI1NiJ9",
        "AIzaSyD-example",
        "-----BEGIN PRIVATE KEY-----",
        "0123456789abcdef0123456789abcdef0",
        "dwie\nlinie",
        "",
        long.as_str(),
    ] {
        assert!(!d.insert("id", value), "wartość `{value}`");
    }
    assert!(d.is_empty());
}

#[test]
fn caps_number_of_entries_but_allows_overwrite() {
    let mut d = ErrorDetails::default();
    for i in 0..MAX_ENTRIES {
        assert!(d.insert(&format!("f{i}"), 1u64));
    }
    assert!(!d.insert("extra", 1u64));
    assert!(d.insert("f0", 2u64));
    assert_eq!(d.number("f0"), Some(2));
}

#[test]
fn deserialization_goes_through_the_same_filter() {
    let d: ErrorDetails = serde_json::from_str(
        r#"{"kind":"camera","api_key":"x","id":"sk-live-1","list":[1],"n":-1,"retry_after_s":5}"#,
    )
    .unwrap();
    let keys: Vec<&str> = d.iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["kind", "retry_after_s"]);
    assert_eq!(
        serde_json::to_string(&d).unwrap(),
        r#"{"kind":"camera","retry_after_s":5}"#
    );
}
