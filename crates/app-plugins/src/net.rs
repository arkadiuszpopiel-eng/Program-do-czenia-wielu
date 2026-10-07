//! Sieć wtyczek (`net.get`): wyłącznie `https://`, host z adresu musi mieścić się w tokenie
//! Brokera `net.egress(host)` (egress-allowlista = zgoda na ten host) i nie może być na deny-liście
//! domen Jądra. Reguły adresów i klient pochodzą z `lib-netguard` (wspólne z `tools-net`): host
//! liczony parserem WHATWG klienta (adresy IP w każdym zapisie — `2130706433`, `0x7f000001`,
//! `127.1` — sprowadzone do postaci kanonicznej i sprawdzone; SR3-02), bez hostów lokalnych,
//! klient bez proxy i bez przekierowań (nowy host = nowa operacja przez Brokera), resolver
//! odrzucający host, gdy którykolwiek adres z DNS jest niepubliczny (DNS rebinding), limit czasu
//! i rozmiaru odpowiedzi.

use std::time::Duration;

use async_trait::async_trait;
use lib_netguard::client::{ClientConfig, SystemLookup, guarded_client, is_guard_rejection};

pub use lib_netguard::is_public_ip;

/// Limit czasu całego żądania.
pub const NET_TIMEOUT: Duration = Duration::from_secs(15);

/// Odpowiedź HTTP (treść tekstowa, obcięta limitem przed odczytem całości).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpsResponse {
    /// Status.
    pub status: u16,
    /// `Content-Type`.
    pub content_type: Option<String>,
    /// Treść (UTF-8).
    pub body: String,
}

/// Pobieranie `GET` (port: produkcyjnie [`EgressClient`], w testach atrapa).
#[async_trait]
pub trait HttpsGet: Send + Sync {
    /// `GET url` z limitem treści `max_bytes`.
    async fn get(&self, url: &str, max_bytes: usize) -> Result<HttpsResponse, String>;
}

/// Host z adresu `https://host[:port]/…` w postaci **kanonicznej**, z jaką połączy się klient
/// (`lib_netguard::check_url`: małe litery, IDNA, adres IP po parserze WHATWG) — `None` dla
/// innych schematów, poświadczeń w adresie, hostów lokalnych i adresów niepublicznych w każdym
/// zapisie (przegląd #3, SR3-02). Klient łączy się z literałem IP bez resolvera, więc to jest
/// jedyna kontrola takich adresów.
pub fn https_host(url: &str) -> Option<String> {
    lib_netguard::check_url(url)
        .ok()
        .map(|t| t.host().to_owned())
}

/// Klient HTTPS wtyczek.
pub struct EgressClient {
    client: reqwest::Client,
}

impl EgressClient {
    /// Klient `lib-netguard`: tylko HTTPS, bez proxy, bez przekierowań, resolver tylko z adresami
    /// publicznymi, limit całego żądania [`NET_TIMEOUT`].
    pub fn new() -> Result<Self, String> {
        let config = ClientConfig {
            connect_timeout: Duration::from_secs(5),
            read_timeout: NET_TIMEOUT,
            total_timeout: Some(NET_TIMEOUT),
            user_agent: "Alfa-plugin/1".into(),
        };
        guarded_client(&config, SystemLookup).map(|client| Self { client })
    }
}

fn request_error(e: reqwest::Error, what: &str) -> String {
    if is_guard_rejection(&e) {
        format!("{what}: host wskazuje adres niepubliczny (sieć lokalna, DNS rebinding)")
    } else {
        format!("{what}: {}", e.without_url())
    }
}

#[async_trait]
impl HttpsGet for EgressClient {
    async fn get(&self, url: &str, max_bytes: usize) -> Result<HttpsResponse, String> {
        if https_host(url).is_none() {
            return Err("dozwolone tylko adresy https:// do hostów publicznych".into());
        }
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| request_error(e, "żądanie nie powiodło się"))?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| request_error(e, "odczyt odpowiedzi"))?
        {
            if body.len().saturating_add(chunk.len()) > max_bytes {
                return Err(format!("odpowiedź większa niż {max_bytes} B"));
            }
            body.extend_from_slice(&chunk);
        }
        let body = String::from_utf8(body).map_err(|_| "odpowiedź nie jest tekstem UTF-8")?;
        Ok(HttpsResponse {
            status,
            content_type,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_https_hosts() {
        assert_eq!(
            https_host("https://Api.Example.com/x?y").as_deref(),
            Some("api.example.com")
        );
        assert_eq!(
            https_host("https://example.com:8443/").as_deref(),
            Some("example.com")
        );
        for bad in [
            "http://example.com/",
            "https://user@example.com/",
            "https://localhost/",
            "https://a.localhost/",
            "https://127.0.0.1/",
            "https://10.1.2.3/",
            "https://192.168.0.1/",
            "https://169.254.169.254/latest/meta-data",
            "https://[::1]/",
            "https://[fe80::1]/",
            "https://[::ffff:127.0.0.1]/",
            "https:///",
            "ftp://example.com/",
        ] {
            assert!(https_host(bad).is_none(), "{bad}");
        }
        assert_eq!(https_host("https://8.8.8.8/").as_deref(), Some("8.8.8.8"));
    }

    #[test]
    fn public_ip_classification() {
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700::1111"] {
            assert!(is_public_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "100.64.0.1",
            "198.18.0.1",
            "0.1.2.3",
            "240.0.0.1",
            "255.255.255.255",
            "224.0.0.1",
            "fd00::1",
            "2001:db8::1",
            "::",
        ] {
            assert!(!is_public_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[tokio::test]
    async fn client_refuses_non_public_targets_without_network() {
        let c = EgressClient::new().unwrap();
        let e = c.get("https://127.0.0.1/", 10).await.unwrap_err();
        assert!(e.contains("https://"), "{e}");
        let e = c.get("http://example.com/", 10).await.unwrap_err();
        assert!(e.contains("https://"), "{e}");
    }
}
