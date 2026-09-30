//! Parsowanie wpisów katalogu: fixture'y, reguły semantyczne, błędy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use accounts_hub_contract::contract_tests::fixture_catalog;
use accounts_hub_contract::{
    AccountState, CatalogError, ModelId, ModelPrice, PriceTable, ProviderCatalogEntry, Tribool,
    validate_base_url,
};
use compliance_contract::{PrivacyTag, ProviderApiStatus};

const BASE: &str = r#"
id = "demo"
display_name = "Demo"
kind = "multi"
auth = "api_key"
base_url = "https://api.demo.example"
compat = "anthropic"
privacy_tag = "sg"
jurisdiction = "SG|EU"
terms_url = "TODO"
compliance_status = "unverified"
notes = ""
[capabilities]
vision = true
tools = false
streaming = "unknown"
long_context = "unknown"
[pricing]
"#;

#[test]
fn parses_fields() {
    let e = ProviderCatalogEntry::from_toml(BASE, "demo").unwrap();
    assert_eq!(e.base_url.as_deref(), Some("https://api.demo.example"));
    assert_eq!(e.terms_url, None);
    assert_eq!(e.capabilities.vision, Tribool::Yes);
    assert_eq!(e.capabilities.tools, Tribool::No);
    assert_eq!(e.capabilities.streaming, Tribool::Unknown);
    assert_eq!(e.compliance_status, ProviderApiStatus::Unverified);
    let policy = e.policy_input();
    assert_eq!(policy.provider, "demo");
    assert!(policy.tags.privacy.contains(&PrivacyTag::Sg));
    assert!(policy.tags.jurisdiction.contains("EU"));
    assert_eq!(fixture_catalog().len(), 4);
}

#[test]
fn rejects_bad_entries() {
    let cases = [
        (BASE.to_owned(), "other", "nazwy pliku"),
        (
            BASE.replace("[pricing]", "[pricing]\nx = 1"),
            "demo",
            "cennik",
        ),
        (
            BASE.replace("https://api.demo.example", "http://api.demo.example"),
            "demo",
            "base_url",
        ),
        (
            BASE.replace("notes = \"\"", "notes = \"\"\nextra = 1"),
            "demo",
            "extra",
        ),
        (
            BASE.replace("notes = \"\"", "notes = \"\"\nenv_vars = [\"bad-name\"]"),
            "demo",
            "zmiennej",
        ),
        (
            BASE.replace("vision = true", "vision = \"yes\""),
            "demo",
            "unknown",
        ),
        (BASE.replace("\"sg\"", "\"public\""), "demo", "public"),
        (BASE.replace("SG|EU", "sg"), "demo", "jurysdykcja"),
        (
            BASE.replace("display_name = \"Demo\"", "display_name = \" \""),
            "demo",
            "nazwa",
        ),
    ];
    for (text, stem, needle) in cases {
        let err = ProviderCatalogEntry::from_toml(&text, stem).unwrap_err();
        assert!(
            matches!(
                err,
                CatalogError::Syntax { .. } | CatalogError::Invalid { .. }
            ),
            "{err}"
        );
        assert!(err.to_string().contains(needle), "{needle}: {err}");
    }
}

#[test]
fn price_table_fallback_and_urls() {
    let mut t = PriceTable::default();
    assert!(t.is_empty());
    let star = ModelId::new("*").unwrap();
    let price = ModelPrice {
        input_micro_usd_per_mtok: 1,
        output_micro_usd_per_mtok: 2,
        ..ModelPrice::default()
    };
    t.0.insert(star, price);
    assert_eq!(t.price_for(&ModelId::new("any").unwrap()), Some(price));
    assert!(validate_base_url("https://x.example").is_ok());
    assert!(validate_base_url("http://127.0.0.1:1234/v1").is_ok());
    assert!(validate_base_url("http://localhost").is_ok());
    for bad in [
        "http://localhost.evil.com",
        "https://",
        "ftp://x",
        "http://10.0.0.1",
    ] {
        assert!(validate_base_url(bad).is_err(), "{bad}");
    }
    assert!(AccountState::Active.is_usable());
}
