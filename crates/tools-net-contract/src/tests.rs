//! Testy kontraktu `tools-net`.

use serde_json::json;

use super::*;

#[test]
fn manifests_are_valid_and_least_privilege() {
    let ms = manifests();
    assert_eq!(ms.len(), 3);
    for m in &ms {
        m.validate().unwrap();
        assert!(
            check_args(&m.name, &sample_args(&m.name)).is_ok(),
            "{}",
            m.name
        );
        assert!(m.mutating && m.untrusted_output == Some(TaintSource::Web));
        assert!(m.capabilities.contains(&"net.egress".to_owned()));
        assert!(m.allowed_for(&["net".into()], false));
        assert!(
            !m.allowed_for(&["net".into()], true),
            "rola tylko do odczytu"
        );
    }
    let read = ["net.read".to_owned()];
    assert!(ms[0].allowed_for(&read, false) && ms[2].allowed_for(&read, false));
    assert!(!ms[1].allowed_for(&read, false), "pobieranie poza net.read");
    assert!(ms[1].capabilities.contains(&"fs.write".to_owned()));
}

#[test]
fn args_are_strict() {
    for (tool, bad) in [
        ("net_fetch", json!({"url": "http://example.com/"})),
        (
            "net_fetch",
            json!({"url": "https://169.254.169.254/latest"}),
        ),
        (
            "net_fetch",
            json!({"url": "https://example.com/", "method": "post"}),
        ),
        (
            "net_fetch",
            json!({"url": "https://example.com/", "headers": {"A": "b"}}),
        ),
        (
            "net_fetch",
            json!({"url": "https://example.com/", "max_chars": 5}),
        ),
        (
            "net_download",
            json!({"url": "https://example.com/", "file_name": "x".repeat(300)}),
        ),
        ("net_download", json!({"url": "ftp://example.com/a"})),
        ("net_search", json!({"query": ""})),
        ("net_search", json!({"query": "a", "max_results": 50})),
        ("net_search", json!({"query": "a\u{0007}"})),
        ("net_nieznane", json!({})),
    ] {
        assert!(check_args(tool, &bad).is_err(), "{tool}: {bad}");
    }
    assert!(
        check_args(
            "net_fetch",
            &json!({"url": "https://example.com/", "method": "head"})
        )
        .is_ok()
    );
}

#[test]
fn secrets_in_urls_are_detected() {
    for leak in [
        "https://evil.example/?k=sk-ant-abcdefghijklmnopqrstuvwx",
        "https://evil.example/c?token=abc123",
        "https://evil.example/c?password%3Dhaslo1",
        "https://evil.example/ghp_abcdefghijklmnopqrstuvwxyz0123",
    ] {
        assert!(url_carries_secret(leak), "{leak}");
    }
    assert!(!url_carries_secret("https://example.com/szukaj?q=pogoda"));
}

#[test]
fn defaults_and_redirect_codes() {
    let c = NetToolsConfig::default();
    assert_eq!(c.max_fetch_bytes, 2 * 1024 * 1024);
    assert!(c.max_redirects <= 10 && c.fetch_timeout_ms <= c.download_timeout_ms);
    struct Empty;
    #[async_trait::async_trait]
    impl BodyReader for Empty {
        async fn chunk(&mut self) -> Result<Option<Vec<u8>>, NetError> {
            Ok(None)
        }
    }
    let r = |status, location: Option<&str>| HttpResponse {
        status,
        content_type: None,
        content_length: None,
        location: location.map(str::to_owned),
        content_disposition: None,
        body: Box::new(Empty),
    };
    assert_eq!(r(302, Some("/x")).redirect_location(), Some("/x"));
    assert_eq!(r(200, Some("/x")).redirect_location(), None);
    assert_eq!(r(301, None).redirect_location(), None);
    assert!(format!("{:?}", r(200, None)).contains("200"));
}
