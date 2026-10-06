//! Sieć wtyczek (`net.get`): wyłącznie `https://`, host z adresu musi mieścić się w tokenie
//! Brokera `net.egress(host)` (egress-allowlista = zgoda na ten host) i nie może być na deny-liście
//! domen Jądra. Klient bez proxy i bez przekierowań (nowy host = nowa operacja przez Brokera),
//! z własnym resolverem odrzucającym adresy niepubliczne (pętla zwrotna, sieci prywatne,
//! link-local — także po DNS), z limitem czasu i rozmiaru odpowiedzi.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

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

/// Czy adres IP jest publiczny (nie: pętla, prywatne, link-local, CGNAT, multicast, dokumentacja,
/// nieokreślony, unikalne lokalne IPv6, IPv4 osadzony w IPv6 — zmapowany, zgodny, NAT64 `64:ff9b::/96`
/// — z takim adresem, lokalny NAT64 `64:ff9b:1::/48`).
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1]))
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            // `::a.b.c.d` (zgodny, przestarzały — także `::1`) i `::ffff:a.b.c.d` (zmapowany).
            if let Some(v4) = v6.to_ipv4() {
                return is_public_ip(IpAddr::V4(v4));
            }
            // NAT64 (RFC 6052): adres IPv4 w ostatnich 32 bitach.
            if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                let v4 = std::net::Ipv4Addr::from((u32::from(s[6]) << 16) | u32::from(s[7]));
                return is_public_ip(IpAddr::V4(v4));
            }
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                || (s[0] == 0x2001 && s[1] == 0x0db8)
                || (s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1))
        }
    }
}

/// Host z adresu `https://host[:port]/…` w postaci **kanonicznej**, z jaką połączy się klient
/// (małe litery, IDNA, adres IP po parserze WHATWG) — `None` dla innych schematów, poświadczeń
/// w adresie, hostów lokalnych i adresów niepublicznych w każdym zapisie (`2130706433`,
/// `0x7f000001`, `0177.0.0.1`, `127.1` = `127.0.0.1`; przegląd #3, SR3-02). Klient łączy się
/// z literałem IP bez resolvera [`PublicOnly`], więc to jest jedyna kontrola takich adresów.
pub fn https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split(['/', '\\', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    let raw = parsed.host_str()?;
    let host = raw
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(raw)
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() || host == "localhost" || host.ends_with(".localhost") {
        return None;
    }
    match host.parse::<IpAddr>() {
        Ok(ip) if !is_public_ip(ip) => None,
        _ => Some(host),
    }
}

/// Resolver odrzucający adresy niepubliczne (DNS rebinding na sieć lokalną).
struct PublicOnly;

impl reqwest::dns::Resolve for PublicOnly {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let found: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 443))
                .await?
                .filter(|a| is_public_ip(a.ip()))
                .collect();
            if found.is_empty() {
                let e: Box<dyn std::error::Error + Send + Sync> =
                    format!("host {host} nie ma publicznego adresu").into();
                return Err(e);
            }
            Ok(Box::new(found.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// Klient HTTPS wtyczek.
pub struct EgressClient {
    client: reqwest::Client,
}

impl EgressClient {
    /// Klient: tylko HTTPS, bez proxy, bez przekierowań, resolver tylko z adresami publicznymi.
    pub fn new() -> Result<Self, String> {
        reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .dns_resolver(Arc::new(PublicOnly))
            .timeout(NET_TIMEOUT)
            .connect_timeout(Duration::from_secs(5))
            .user_agent("Alfa-plugin/1")
            .build()
            .map(|client| Self { client })
            .map_err(|e| e.to_string())
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
            .map_err(|e| format!("żądanie nie powiodło się: {}", e.without_url()))?;
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
            .map_err(|e| format!("odczyt odpowiedzi: {}", e.without_url()))?
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
