//! Adres docelowy od modelu albo z przekierowania: parser WHATWG (`url`, ten sam co w `reqwest`),
//! tylko `https://`, bez danych logowania, bez hostów lokalnych i adresów IP niepublicznych.

use std::net::IpAddr;

use url::{Host, Url};

use crate::addr::is_public_ip;

/// Najdłuższy przyjmowany adres (znaki).
pub const MAX_URL_LEN: usize = 2_048;

/// Domeny najwyższego poziomu / sufiksy sieci lokalnych (mDNS, intranet, routery domowe).
const LOCAL_SUFFIXES: [&str; 12] = [
    "localhost",
    "local",
    "localdomain",
    "internal",
    "intranet",
    "lan",
    "home",
    "corp",
    "private",
    "home.arpa",
    "in-addr.arpa",
    "ip6.arpa",
];

/// Dlaczego adres odrzucono.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UrlError {
    /// Za długi.
    #[error("adres dłuższy niż {MAX_URL_LEN} znaków")]
    TooLong,
    /// Nie da się sparsować albo zawiera znaki niedozwolone.
    #[error("niepoprawny adres: {0}")]
    Invalid(String),
    /// Schemat inny niż `https`.
    #[error("dozwolony jest wyłącznie schemat https:// (otrzymano {0}://)")]
    NotHttps(String),
    /// Dane logowania w adresie (`user:hasło@`).
    #[error("adres zawiera dane logowania — niedozwolone")]
    Credentials,
    /// Brak hosta.
    #[error("adres bez hosta")]
    NoHost,
    /// Host lokalny (`localhost`, `*.local`, nazwa jednoczłonowa…).
    #[error("host lokalny {0} — dozwolone tylko hosty publiczne")]
    LocalHost(String),
    /// Adres IP niepubliczny.
    #[error("adres IP niepubliczny {0} — dozwolone tylko adresy publiczne")]
    PrivateIp(IpAddr),
}

/// Sprawdzony adres docelowy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    url: Url,
    host: String,
    port: u16,
}

impl Target {
    /// Adres (bez fragmentu).
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Adres jako tekst.
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    /// Host (małe litery, punycode; adres IPv6 bez nawiasów).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Port (domyślnie 443).
    pub fn port(&self) -> u16 {
        self.port
    }
}

fn is_local_domain(domain: &str) -> bool {
    !domain.contains('.')
        || LOCAL_SUFFIXES
            .iter()
            .any(|s| domain == *s || domain.ends_with(&format!(".{s}")))
}

fn check_raw(raw: &str) -> Result<(), UrlError> {
    if raw.chars().count() > MAX_URL_LEN {
        return Err(UrlError::TooLong);
    }
    if raw.chars().any(|c| c.is_control() || c == '\\') {
        return Err(UrlError::Invalid(
            "znaki sterujące albo ukośnik wsteczny w adresie".into(),
        ));
    }
    Ok(())
}

fn check_parsed(mut url: Url) -> Result<Target, UrlError> {
    if url.scheme() != "https" {
        return Err(UrlError::NotHttps(url.scheme().to_owned()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(UrlError::Credentials);
    }
    let host = match url.host() {
        None => return Err(UrlError::NoHost),
        Some(Host::Ipv4(v4)) => {
            let ip = IpAddr::V4(v4);
            if !is_public_ip(ip) {
                return Err(UrlError::PrivateIp(ip));
            }
            v4.to_string()
        }
        Some(Host::Ipv6(v6)) => {
            let ip = IpAddr::V6(v6);
            if !is_public_ip(ip) {
                return Err(UrlError::PrivateIp(ip));
            }
            v6.to_string()
        }
        Some(Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            if d.is_empty() || is_local_domain(&d) {
                return Err(UrlError::LocalHost(d));
            }
            d
        }
    };
    let port = url.port_or_known_default().unwrap_or(443);
    url.set_fragment(None);
    Ok(Target { url, host, port })
}

/// Sprawdza adres od modelu.
pub fn check_url(raw: &str) -> Result<Target, UrlError> {
    check_raw(raw)?;
    let raw = raw.trim();
    let url = Url::parse(raw).map_err(|e| UrlError::Invalid(e.to_string()))?;
    // WHATWG przyjmuje `https:host` i `https:/host` jak `https://host` — od modelu i wtyczek
    // wymagamy jawnej postaci (adres widziany przez właściciela = adres użyty).
    let explicit = raw
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("https://"));
    if url.scheme() == "https" && !explicit {
        return Err(UrlError::Invalid("po `https:` wymagane `//`".into()));
    }
    check_parsed(url)
}

/// Adres z nagłówka `Location` względem bieżącego — te same reguły (bez obniżenia do `http`).
pub fn redirect_target(base: &Url, location: &str) -> Result<Target, UrlError> {
    check_raw(location)?;
    let url = base
        .join(location.trim())
        .map_err(|e| UrlError::Invalid(e.to_string()))?;
    check_parsed(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_public_https() {
        let t = check_url("https://Example.COM./a/b?c=1#frag").unwrap();
        assert_eq!(t.host(), "example.com");
        assert_eq!(t.port(), 443);
        assert_eq!(t.as_str(), "https://example.com./a/b?c=1");
        let t = check_url("https://xn--d1acufc.xn--p1ai:8443/").unwrap();
        assert_eq!(t.port(), 8443);
        let t = check_url("https://żółw.pl/").unwrap();
        assert_eq!(t.host(), "xn--w-uga1v8h.pl");
        assert_eq!(check_url("https://8.8.8.8/").unwrap().host(), "8.8.8.8");
        assert_eq!(check_url("HTTPS://134744072/").unwrap().host(), "8.8.8.8");
        assert_eq!(
            check_url("https://[2606:4700::1111]/").unwrap().host(),
            "2606:4700::1111"
        );
    }

    #[test]
    fn rejects_everything_else() {
        let long = format!("https://a.pl/{}", "x".repeat(MAX_URL_LEN));
        for (bad, why) in [
            ("http://example.com/", "http"),
            ("ftp://example.com/", "ftp"),
            ("file:///C:/Windows/win.ini", "file"),
            ("javascript:alert(1)", "js"),
            ("https://user:pass@example.com/", "dane logowania"),
            ("https://user@example.com/", "użytkownik"),
            ("https://localhost/", "localhost"),
            ("https://LOCALHOST./", "localhost z kropką"),
            ("https://a.localhost/", "*.localhost"),
            ("https://drukarka.local/", "mDNS"),
            ("https://serwer.internal/", "internal"),
            ("https://router.home.arpa/", "home.arpa"),
            ("https://intranet/", "jednoczłonowy"),
            ("https://127.0.0.1/", "pętla"),
            ("https://127.1/", "pętla skrócona"),
            ("https://0x7f.0.0.1/", "pętla szesnastkowo"),
            ("https://2130706433/", "pętla dziesiętnie"),
            ("https://017700000001/", "pętla ósemkowo"),
            ("https://10.0.0.1/", "prywatny"),
            (
                "https://169.254.169.254/latest/meta-data/",
                "metadane chmury",
            ),
            ("https://[::1]/", "IPv6 pętla"),
            ("https://[::ffff:127.0.0.1]/", "IPv4 mapowany"),
            ("https://[fe80::1]/", "link-local"),
            ("https://[fd00::1]/", "ULA"),
            ("https://0.0.0.0/", "nieokreślony"),
            ("https:///x", "bez hosta"),
            ("https://exa mple.com/", "spacja"),
            ("https:example.com", "bez `//`"),
            ("https:/example.com/", "jeden ukośnik"),
            ("https://0/", "0.0.0.0 skrótem"),
            ("https://10.1/", "prywatny skrótem"),
            ("https://3232235777/", "192.168.1.1 dziesiętnie"),
            ("https://0xa9fea9fe/", "metadane chmury szesnastkowo"),
            ("https://[::127.0.0.1]/", "IPv4 zgodny z IPv6"),
            ("https://[64:ff9b::a00:1]/", "NAT64 prywatnego"),
            ("https://example.com\\@evil.com/", "ukośnik wsteczny"),
            ("https://example.com/\r\nHost: x", "CRLF"),
            (long.as_str(), "za długi"),
        ] {
            assert!(check_url(bad).is_err(), "{why}: {bad}");
        }
    }

    #[test]
    fn redirects_keep_rules() {
        let base = check_url("https://a.example.com/x/y").unwrap();
        let t = redirect_target(base.url(), "/z").unwrap();
        assert_eq!(t.as_str(), "https://a.example.com/z");
        let t = redirect_target(base.url(), "//cdn.example.net/f").unwrap();
        assert_eq!(t.host(), "cdn.example.net");
        for bad in [
            "http://a.example.com/z",
            "https://127.0.0.1/",
            "https://[::1]/",
            "https://localhost/",
            "//10.0.0.5/admin",
            "https://u:p@a.example.com/",
            "file:///etc/passwd",
        ] {
            assert!(redirect_target(base.url(), bad).is_err(), "{bad}");
        }
    }
}
