//! Normalizacja hosta (z samej domeny albo z URL-a) i dopasowanie do deny-listy domen.

use super::path::percent_decode;

const SPECIAL_SCHEMES: [&str; 6] = ["http", "https", "ws", "wss", "ftp", "file"];

/// Wyciąga i normalizuje host: małe litery, bez schematu, userinfo, portu, ścieżki,
/// z dekodowaniem `%XX`, kropkami unikodowymi zamienionymi na `.`, bez kropek brzegowych,
/// a host z nie-ASCII — po mapowaniu IDNA (UTS 46) do postaci ASCII. `None`, gdy nie da się
/// wyciągnąć niepustego hosta albo host z nie-ASCII nie przechodzi mapowania IDNA.
pub fn normalize_host(input: &str) -> Option<String> {
    let lowered = input.trim().to_lowercase().replace('\\', "/");
    let mut s = lowered.as_str();
    if let Some((scheme, rest)) = s.split_once(':')
        && SPECIAL_SCHEMES.contains(&scheme)
    {
        s = rest;
    }
    s = s.trim_start_matches('/');
    let end = s.find(['/', '?', '#']).unwrap_or(s.len());
    s = &s[..end];
    if let Some(at) = s.rfind('@') {
        s = &s[at + 1..];
    }
    let host = if let Some(v6) = s.strip_prefix('[') {
        v6.split(']').next().unwrap_or_default().to_owned()
    } else {
        match s.rsplit_once(':') {
            Some((h, port)) if port.chars().all(|c| c.is_ascii_digit()) => h.to_owned(),
            _ => s.to_owned(),
        }
    };
    let decoded = percent_decode(&host).to_lowercase();
    let dotted: String = decoded
        .chars()
        .map(|c| match c {
            '\u{3002}' | '\u{ff0e}' | '\u{ff61}' => '.',
            other => other,
        })
        .collect();
    let trimmed = dotted.trim_matches('.').trim();
    // Host z nie-ASCII: mapowanie UTS 46 / WHATWG „domain to ASCII” — to samo, co zrobi klient
    // HTTP i WebView2 (`ⅽlaude.ai`, pełna szerokość, litery matematyczne → `claude.ai`; IDN →
    // punycode). Bez tego deny-lista porównywałaby inną postać niż ta, z którą się połączymy.
    let mapped = if trimmed.is_ascii() {
        trimmed.to_owned()
    } else {
        idna::domain_to_ascii(trimmed).ok()?
    };
    let host = mapped.trim_matches('.');
    (!host.is_empty() && !host.contains(char::is_whitespace)).then(|| host.to_owned())
}

/// Czy host (już znormalizowany) to domena z listy albo jej subdomena.
pub fn host_matches(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_host_from_urls() {
        let cases = [
            ("claude.ai", "claude.ai"),
            ("CLAUDE.AI.", "claude.ai"),
            ("https://claude.ai/chat/1?x=2#y", "claude.ai"),
            ("https://user:pw@www.claude.ai:443/", "www.claude.ai"),
            ("https:\\\\claude.ai\\x", "claude.ai"),
            ("http:claude.ai", "claude.ai"),
            ("//claude.ai/x", "claude.ai"),
            ("https://claude%2Eai/", "claude.ai"),
            ("claude\u{3002}ai", "claude.ai"),
            ("https://[::1]:8080/", "::1"),
        ];
        for (input, want) in cases {
            assert_eq!(normalize_host(input).as_deref(), Some(want), "{input}");
        }
        assert_eq!(normalize_host("   "), None);
        assert_eq!(normalize_host("https:///"), None);
    }

    #[test]
    fn subdomain_matching() {
        assert!(host_matches("claude.ai", "claude.ai"));
        assert!(host_matches("www.claude.ai", "claude.ai"));
        assert!(!host_matches("notclaude.ai", "claude.ai"));
        assert!(!host_matches("claude.ai.evil.com", "claude.ai"));
    }
}
