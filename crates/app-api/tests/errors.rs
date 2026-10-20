//! Błędy IPC (`AppError`): kody, szczegóły bez sekretów, kształt na drucie, tłumaczenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_api::{AppError, ErrorCode};

#[test]
fn unavailable_names_module_and_localizes() {
    let e = AppError::unavailable("Import paczki", "transfer");
    assert_eq!(e.code, ErrorCode::Unavailable);
    assert!(e.message.contains("modułu transfer"));
    assert!(e.localized("en").starts_with("This feature"));
    assert_eq!(e.localized("pl"), e.message);
}

#[test]
fn session_errors_map_to_codes() {
    let nf: AppError = sessions_contract::SessionError::NotFound {
        id: sessions_contract::SessionId::new("x"),
    }
    .into();
    assert_eq!(nf.code, ErrorCode::NotFound);
    let bad: AppError = sessions_contract::SessionError::EmptyTurn.into();
    assert_eq!(bad.code, ErrorCode::InvalidInput);
}

#[test]
fn details_are_absent_on_the_wire_when_empty() {
    let plain = AppError::invalid("Zła wartość.");
    assert_eq!(
        serde_json::to_string(&plain).unwrap(),
        r#"{"code":"invalid_input","message":"Zła wartość."}"#
    );
    // Kształt sprzed B.3 (bez `details`) nadal się wczytuje.
    let old: AppError =
        serde_json::from_str(r#"{"code":"not_found","message":"Nie ma."}"#).unwrap();
    assert_eq!(old, AppError::not_found("Nie ma."));
    assert!(old.details.is_empty());
}

#[test]
fn details_roundtrip_with_stable_keys() {
    let e = AppError::provider_busy("anthropic", Some(30), "Limit zapytań.");
    let json = serde_json::to_value(&e).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "code": "provider",
            "message": "Limit zapytań.",
            "details": { "provider": "anthropic", "reason": "rate_limited", "retry_after_s": 30 }
        })
    );
    assert_eq!(serde_json::from_value::<AppError>(json).unwrap(), e);
    let unknown = AppError::provider_busy("openai", None, "Przeciążenie.");
    assert_eq!(unknown.details.get("retry_after_s"), None);
}

#[test]
fn constructors_fill_details() {
    let missing = AppError::missing("camera", "cam-1", "Nie ma takiej kamery.");
    assert_eq!(missing.code, ErrorCode::NotFound);
    assert_eq!(missing.details.text("kind"), Some("camera"));
    assert_eq!(missing.details.text("id"), Some("cam-1"));
    let field = AppError::invalid_field("title", "Zły tytuł.");
    assert_eq!(field.details.text("field"), Some("title"));
    let module = AppError::unavailable("Import paczki", "transfer");
    assert_eq!(module.details.text("module"), Some("transfer"));
    let nf: AppError = sessions_contract::SessionError::NotFound {
        id: sessions_contract::SessionId::new("s-1"),
    }
    .into();
    assert_eq!(nf.details.text("kind"), Some("session"));
    assert_eq!(nf.details.text("id"), Some("s-1"));
}

#[test]
fn details_never_carry_secrets() {
    // Wszystkie konstruktory idą przez `with`, a ten przez filtr z `details.rs`.
    let e = AppError::invalid("x")
        .with("api_key", "abc")
        .with("token", "abc")
        .with("id", "sk-ant-api03-zzzz")
        .with("field", "0123456789abcdef0123456789abcdef0123456789abcdef");
    assert!(e.details.is_empty(), "{:?}", e.details);
    let via_ctor = AppError::missing("account", "Bearer abc.def.ghi", "Nie ma konta.");
    assert_eq!(via_ctor.details.text("id"), None);
    assert_eq!(via_ctor.details.text("kind"), Some("account"));
    assert!(!serde_json::to_string(&via_ctor).unwrap().contains("Bearer"));
}
