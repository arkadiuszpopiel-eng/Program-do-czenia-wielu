//! Przegląd bezpieczeństwa #3 (2026-10, SR3-02): sieć wtyczek (`net.get`) nie może dosięgnąć
//! adresów niepublicznych przez **nieskanoniczne zapisy adresu IP** w adresie. Klient (`reqwest`,
//! parser WHATWG `url`) zamienia hosty `2130706433`, `0x7f000001`, `0177.0.0.1`, `127.1` na
//! `127.0.0.1` i łączy się z nim **bez** resolvera `PublicOnly` (resolver dotyczy tylko nazw
//! domenowych), więc sprawdzenie hosta musi używać tego samego parsera co klient.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_plugins::net::{EgressClient, HttpsGet, https_host, is_public_ip};

/// Zapisy adresów pętli zwrotnej, sieci prywatnych, link-local i metadanych chmury, które parser
/// URL klienta sprowadza do adresu niepublicznego.
const NON_PUBLIC: [&str; 14] = [
    "https://2130706433/",        // 127.0.0.1 dziesiętnie
    "https://0x7f000001/",        // 127.0.0.1 szesnastkowo
    "https://0x7f.1/",            // 127.0.0.1 skrótem z szesnastkową oktetą
    "https://0177.0.0.1/",        // 127.0.0.1 ósemkowo
    "https://127.1/",             // 127.0.0.1 skrótem
    "https://0/",                 // 0.0.0.0
    "https://10.1/",              // 10.0.0.1
    "https://192.168.1/",         // 192.168.0.1
    "https://3232235777/",        // 192.168.1.1
    "https://2852039166/latest/", // 169.254.169.254 (metadane chmury)
    "https://0xa9fea9fe/latest/", // 169.254.169.254
    "https://[::127.0.0.1]/",     // IPv4 zgodny z IPv6 (przestarzały) → 127.0.0.1
    "https://[::7f00:1]/",        // to samo
    "https://[64:ff9b::a00:1]/",  // NAT64 → 10.0.0.1
];

#[test]
fn non_canonical_ip_literals_are_not_public_hosts() {
    for url in NON_PUBLIC {
        assert_eq!(https_host(url), None, "{url} wskazuje adres niepubliczny");
    }
}

#[test]
fn public_hosts_still_pass_in_canonical_form() {
    assert_eq!(
        https_host("https://Api.Example.com/x?y").as_deref(),
        Some("api.example.com")
    );
    assert_eq!(
        https_host("https://example.com.:8443/").as_deref(),
        Some("example.com")
    );
    assert_eq!(https_host("https://8.8.8.8/").as_deref(), Some("8.8.8.8"));
    // Ten sam adres publiczny zapisany dziesiętnie — host kanoniczny (taki, z jakim połączy się klient).
    assert_eq!(https_host("https://134744072/").as_deref(), Some("8.8.8.8"));
    assert_eq!(
        https_host("https://[2606:4700::1111]/").as_deref(),
        Some("2606:4700::1111")
    );
    for bad in [
        "https://user@example.com/",
        "https://:pw@example.com/",
        "https://localhost./",
        "https://LOCALHOST/",
        "https://a.localhost/",
        "http://example.com/",
        "https:example.com",
    ] {
        assert_eq!(https_host(bad), None, "{bad}");
    }
}

#[test]
fn embedded_ipv4_in_ipv6_is_classified_by_its_ipv4() {
    for ip in [
        "::127.0.0.1",
        "::a00:1",
        "64:ff9b::7f00:1",
        "64:ff9b::c0a8:101",
    ] {
        assert!(!is_public_ip(ip.parse().unwrap()), "{ip}");
    }
    assert!(is_public_ip("64:ff9b::808:808".parse().unwrap()));
}

#[tokio::test]
async fn client_refuses_numeric_loopback_before_connecting() {
    // Przed poprawką klient łączył się z 127.0.0.1:443 (błąd połączenia, nie odmowa polityki).
    let client = EgressClient::new().unwrap();
    for url in [
        "https://2130706433/",
        "https://127.1/",
        "https://0x7f000001/",
    ] {
        let e = client.get(url, 16).await.unwrap_err();
        assert!(
            e.contains("hostów publicznych"),
            "{url}: oczekiwana odmowa polityki przed połączeniem, jest „{e}”"
        );
    }
}
