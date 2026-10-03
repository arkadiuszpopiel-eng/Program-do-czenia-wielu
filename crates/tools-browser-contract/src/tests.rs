//! Testy kontraktu `tools-browser`.

use serde_json::json;

use super::*;

#[test]
fn manifests_are_valid() {
    let ms = manifests();
    assert_eq!(ms.len(), 6);
    for m in &ms {
        m.validate().unwrap();
        assert_eq!(m.capabilities, vec!["net.egress".to_owned()]);
        assert!(
            check_args(&m.name, &sample_args(&m.name)).is_ok(),
            "{}",
            m.name
        );
    }
    let open = &ms[0];
    assert!(open.mutating && open.untrusted_output == Some(TaintSource::Web));
    assert!(!ms[1].mutating && ms[1].allowed_for(&["browser.read".into()], true));
    assert!(
        !open.allowed_for(&["browser".into()], true),
        "rola tylko do odczytu"
    );
    assert_eq!(ms[5].untrusted_output, None);
}

#[test]
fn args_and_hosts() {
    for bad in [
        json!({"url": "file:///etc/passwd"}),
        json!({"url": "javascript:alert(1)"}),
        json!({"url": "https://a:b@x.pl/"}),
        json!({"url": "https://x.pl/", "extra_hosts": ["*.cdn.net"]}),
        json!({"url": "https://x.pl/", "cookies": true}),
    ] {
        assert!(check_args("browser_open", &bad).is_err(), "{bad}");
    }
    assert!(
        check_args(
            "browser_open",
            &json!({"url": "https://x.pl/", "extra_hosts": ["cdn.x.pl", "https://api.x.pl/v1"]})
        )
        .is_ok()
    );
    assert!(
        check_args(
            "browser_type",
            &json!({"node": 1, "text": "x".repeat(20_000)})
        )
        .is_err()
    );
    assert!(check_args("browser_close", &json!({"x": 1})).is_err());
    assert!(check_args("browser_nic", &json!({})).is_err());
    assert_eq!(
        normalize_host("CDN.Example.com"),
        Some("cdn.example.com".into())
    );
    assert_eq!(
        normalize_host("https://api.x.pl:8443/v1"),
        Some("api.x.pl".into())
    );
    assert_eq!(normalize_host("intranet"), None);
    assert_eq!(normalize_host("*.x.pl"), None);
    let text = render_nodes(&[
        NodeOut {
            node: 1,
            depth: 0,
            role: "link".into(),
            name: "Start".into(),
            value: None,
            password: false,
        },
        NodeOut {
            node: 2,
            depth: 1,
            role: "textbox".into(),
            name: "Hasło".into(),
            value: None,
            password: true,
        },
    ]);
    assert!(text.contains("[1] link „Start”") && text.contains("[pole hasła]"));
}
