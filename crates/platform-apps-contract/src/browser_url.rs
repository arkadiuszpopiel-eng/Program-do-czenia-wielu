//! Adresy w przeglądarce Alfy: host z adresu, adresy bez sieci, sprawdzenie adresu nawigacji
//! od agentki (tylko `http`/`https`, bez danych logowania).

use crate::browser::{BrowserError, MAX_URL_LEN};

/// Host z adresu sieciowego (`http`, `https`, `ws`, `wss`), małymi literami, bez portu.
/// `None` dla schematów bez sieci albo niepoprawnych adresów.
pub fn url_host(url: &str) -> Option<String> {
    let (scheme, rest) = url.trim().split_once("://")?;
    if !matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "ws" | "wss"
    ) {
        return None;
    }
    // Jak WHATWG URL dla schematów specjalnych: `\` to separator ścieżki (ten sam host co Chromium).
    let rest = rest.replace('\\', "/");
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next()?.to_owned()
    } else {
        host_port.split(':').next()?.to_owned()
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Czy żądanie nie wychodzi do sieci (`data:`, `blob:`, `about:blank`).
pub fn is_local_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("data:") || u.starts_with("blob:") || u == "about:blank"
}

/// Adres nawigacji od agentki: tylko `http`/`https`, z hostem, bez danych logowania w URL.
pub fn check_navigation_url(url: &str) -> Result<String, BrowserError> {
    let u = url.trim();
    let lower = u.to_ascii_lowercase();
    if u.len() > MAX_URL_LEN || u.chars().any(char::is_control) {
        return Err(BrowserError::Policy("niepoprawny adres".into()));
    }
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(BrowserError::Policy(
            "dozwolone tylko adresy http:// i https://".into(),
        ));
    }
    let normalized = u.replace('\\', "/");
    let authority = normalized
        .split_once("://")
        .map_or("", |(_, r)| r.split(['/', '?', '#']).next().unwrap_or(""));
    if authority.contains('@') {
        return Err(BrowserError::Policy(
            "adres z danymi logowania jest zablokowany".into(),
        ));
    }
    url_host(u).ok_or_else(|| BrowserError::Policy("adres bez hosta".into()))
}
