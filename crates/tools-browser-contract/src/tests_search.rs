//! Fraza pozostaje pojedynczą wartością q na wybranym znanym hoście; bezAPI i własnych endpointów.
use super::*;
use platform_apps_contract::{MAX_URL_LEN, url_host};
use serde_json::json;

#[test]
fn every_engine_uses_https_and_only_one_encoded_query() {
    let query = "żółć + &q=inne#fragment?x=/\\ @evil.example";
    for (engine, host) in [
        (SearchEngine::Duckduckgo, "duckduckgo.com"),
        (SearchEngine::Google, "www.google.com"),
        (SearchEngine::Bing, "www.bing.com"),
    ] {
        let n = SearchArgs {
            engine,
            query: query.into(),
        }
        .navigation()
        .unwrap();
        assert!(n.url.starts_with("https://"));
        assert_eq!(url_host(&n.url).as_deref(), Some(host));
        assert!(!n.url.contains('#'));
        let pairs: Vec<_> =
            form_urlencoded::parse(n.url.split_once('?').unwrap().1.as_bytes()).collect();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "q");
        assert_eq!(pairs[0].1, query);
        assert!(n.extra_hosts.is_empty());
    }
}

#[test]
fn existing_url_budget_applies_after_utf8_percent_encoding() {
    let base = "https://duckduckgo.com/?q=".len();
    let valid = SearchArgs {
        engine: SearchEngine::Duckduckgo,
        query: "a".repeat(MAX_URL_LEN - base),
    };
    assert_eq!(valid.navigation().unwrap().url.len(), MAX_URL_LEN);
    let longer = SearchArgs {
        query: format!("{}a", valid.query),
        ..valid
    };
    assert!(longer.navigation().is_err());
    let unicode = SearchArgs {
        engine: SearchEngine::Duckduckgo,
        query: "ż".repeat(800),
    };
    assert!(unicode.query.len() < MAX_URL_LEN);
    assert!(unicode.navigation().is_err());
}

#[test]
fn missing_engine_unknown_endpoint_and_bad_queries_are_rejected_without_echoing_query() {
    for args in [
        json!({"query":"Rust"}),
        json!({"engine":"unknown","query":"Rust"}),
        json!({"engine":"duckduckgo","query":"Rust","endpoint":"https://evil.example"}),
        json!({"engine":"duckduckgo","query":"a\nb"}),
        json!({"engine":"duckduckgo","query":""}),
        json!({"engine":"duckduckgo","query":" \t "}),
        json!({"engine":"duckduckgo","query":"marker\u{0}value"}),
        json!({"engine":"duckduckgo","query":"a".repeat(MAX_URL_LEN+1)}),
    ] {
        let e = check_args("browser_search", &args).unwrap_err();
        assert!(!e.contains("marker"));
    }
    let n = SearchArgs {
        engine: SearchEngine::Duckduckgo,
        query: "  Rust async  ".into(),
    }
    .navigation()
    .unwrap();
    assert_eq!(n.url, "https://duckduckgo.com/?q=Rust+async");
}

#[test]
fn search_is_a_mutating_web_tool_and_cannot_be_called_by_read_only_roles() {
    let m = manifests()
        .into_iter()
        .find(|m| m.name == "browser_search")
        .unwrap();
    assert_eq!(m.capabilities, ["net.egress"]);
    assert_eq!(m.untrusted_output, Some(TaintSource::Web));
    assert!(m.mutating);
    assert!(!m.allowed_for(&["browser.read".into()], false));
    assert!(!m.allowed_for(&["browser".into()], true));
    assert!(m.allowed_for(&["browser".into()], false));
}
