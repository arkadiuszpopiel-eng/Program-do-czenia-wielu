//! Polityka adresów URL w linkach i obrazach (ADR 0009, THREAT_MODEL S12).
//!
//! Sprawdzamy dokładnie ten ciąg, który zobaczy przeglądarka: najpierw usuwamy znaki, które parser
//! URL i tak pomija (TAB/LF/CR wszędzie, znaki sterujące C0 i spacje na brzegach), potem czytamy
//! schemat wg WHATWG. Adres bez poprawnego schematu jest względny — a względne odrzucamy, bo
//! w WebView rozwiązałyby się do źródła aplikacji.

use std::ops::Range;

use crate::RenderOptions;

/// Usuwa znaki pomijane przez parser URL przeglądarki.
fn browser_clean(raw: &str) -> String {
    let no_breaks: String = raw
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    no_breaks.trim_matches(|c: char| c <= ' ').to_owned()
}

/// Schemat (małymi literami) albo `None`, gdy adres nie ma poprawnego schematu (= względny).
fn scheme(url: &str) -> Option<String> {
    let colon = url.find(':')?;
    let candidate = &url[..colon];
    let mut chars = candidate.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    Some(candidate.to_ascii_lowercase())
}

/// Czy po `scheme:` jest `//` i niepusty host.
fn has_authority(url: &str) -> bool {
    let Some((_, rest)) = url.split_once(':') else {
        return false;
    };
    let rest = rest.replace('\\', "/");
    rest.strip_prefix("//")
        .and_then(|host| host.chars().next())
        .is_some_and(|c| !matches!(c, '/' | '?' | '#' | '@' | ':'))
}

/// Oczyszczony adres, o ile nie zawiera znaków sterujących.
fn cleaned(raw: &str) -> Option<String> {
    let url = browser_clean(raw);
    (!url.is_empty() && !url.chars().any(char::is_control)).then_some(url)
}

/// Bezpieczny `href` linku: tylko `http(s)://host…` i `mailto:…`; w pozostałych przypadkach `None`
/// (`javascript:`, `data:`, `vbscript:`, `file:`, adresy względne i protokołowo-względne).
pub fn safe_link_href(raw: &str) -> Option<String> {
    let url = cleaned(raw)?;
    match scheme(&url)?.as_str() {
        "http" | "https" => has_authority(&url).then_some(url),
        "mailto" => (url.len() > "mailto:".len()).then_some(url),
        _ => None,
    }
}

/// Czy adres to obraz rastrowy `data:image/(png|jpeg|webp);base64,…` (bez SVG — SVG może nieść skrypt).
pub fn is_safe_data_image(url: &str) -> bool {
    ["png", "jpeg", "webp"].iter().any(|mime| {
        let prefix = format!("data:image/{mime};base64,");
        url.len() > prefix.len()
            && url.is_char_boundary(prefix.len())
            && url[..prefix.len()].eq_ignore_ascii_case(&prefix)
            && url[prefix.len()..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
    })
}

/// Bezpieczny `src` obrazu wg opcji: `https://` (gdy `allow_remote_images`) albo rastrowy
/// `data:image/…;base64` (gdy `allow_data_images`). Domyślnie oba wyłączone → zawsze `None`.
pub fn safe_image_src(raw: &str, opts: &RenderOptions) -> Option<String> {
    let url = cleaned(raw)?;
    match scheme(&url)?.as_str() {
        "https" if opts.allow_remote_images && has_authority(&url) => Some(url),
        "data" if opts.allow_data_images && is_safe_data_image(&url) => Some(url),
        _ => None,
    }
}

/// Czy znak przed kandydatem na autolink pozwala go rozpoznać (GFM: początek, biały znak, `*_~(`).
fn at_boundary(text: &str, i: usize) -> bool {
    text[..i]
        .chars()
        .next_back()
        .is_none_or(|c| c.is_whitespace() || matches!(c, '*' | '_' | '~' | '('))
}

/// Obcina końcową interpunkcję i niezrównoważone nawiasy (GFM „extended autolink”).
fn trim_trailing(candidate: &str) -> &str {
    let mut s = candidate;
    loop {
        let Some(last) = s.chars().next_back() else {
            return s;
        };
        let cut = match last {
            '?' | '!' | '.' | ',' | ':' | '*' | '_' | '~' | '\'' | '"' | ';' => true,
            ')' => s.matches(')').count() > s.matches('(').count(),
            _ => false,
        };
        if !cut {
            return s;
        }
        s = &s[..s.len() - last.len_utf8()];
    }
}

/// Wyszukuje „gołe” adresy (`https://…`, `http://…`, `www.…`) w zwykłym tekście.
/// Zwraca zakresy bajtowe i docelowy `href` (dla `www.` — z prefiksem `https://`).
pub fn find_autolinks(text: &str) -> Vec<(Range<usize>, String)> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &lower[i..];
        let prefix_len = ["https://", "http://", "www."]
            .iter()
            .find(|p| rest.starts_with(**p))
            .map_or(0, |p| p.len());
        if prefix_len > 0 && at_boundary(text, i) {
            let end = text[i..]
                .char_indices()
                .find(|(_, c)| c.is_whitespace() || c.is_control() || *c == '<')
                .map_or(text.len(), |(off, _)| i + off);
            let candidate = trim_trailing(&text[i..end]);
            let body_ok = candidate
                .get(prefix_len..)
                .and_then(|b| b.chars().next())
                .is_some_and(char::is_alphanumeric);
            if body_ok {
                let href = if prefix_len == "www.".len() {
                    format!("https://{candidate}")
                } else {
                    candidate.to_owned()
                };
                if let Some(href) = safe_link_href(&href) {
                    let stop = i + candidate.len();
                    out.push((i..stop, href));
                    i = stop;
                    continue;
                }
            }
        }
        i += text[i..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_schemes() {
        assert_eq!(
            safe_link_href("https://example.com/a").as_deref(),
            Some("https://example.com/a")
        );
        assert!(safe_link_href("mailto:a@b.pl").is_some());
        for bad in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "jav\tascript:alert(1)",
            " \u{1}javascript:alert(1)",
            "java\u{0}script:alert(1)",
            "vbscript:x",
            "data:text/html,x",
            "file:///C:/x",
            "//evil.example",
            "/relative",
            "#frag",
            "%6Aavascript:alert(1)",
            "java\u{200b}script:alert(1)",
            "https:evil",
            "https://",
            "mailto:",
            "",
        ] {
            assert_eq!(safe_link_href(bad), None, "{bad:?}");
        }
        assert_eq!(
            safe_link_href("ht\ntps://ok.pl").as_deref(),
            Some("https://ok.pl")
        );
    }

    #[test]
    fn images_follow_options() {
        let png = "data:image/png;base64,iVBORw0KGgo=";
        let none = RenderOptions::default();
        assert_eq!(safe_image_src(png, &none), None);
        assert_eq!(safe_image_src("https://x.pl/a.png", &none), None);
        let all = RenderOptions {
            allow_data_images: true,
            allow_remote_images: true,
        };
        assert!(safe_image_src(png, &all).is_some());
        assert!(safe_image_src("https://x.pl/a.png", &all).is_some());
        assert_eq!(
            safe_image_src("data:image/svg+xml;base64,PHN2Zz4=", &all),
            None
        );
        assert_eq!(safe_image_src("data:image/png;base64,<x>", &all), None);
        assert_eq!(safe_image_src("http://x.pl/a.png", &all), None);
    }

    #[test]
    fn autolinks() {
        let found = find_autolinks("Zobacz https://example.com/a_(b). i www.rust-lang.org!");
        let hrefs: Vec<_> = found.iter().map(|(_, h)| h.as_str()).collect();
        assert_eq!(
            hrefs,
            ["https://example.com/a_(b)", "https://www.rust-lang.org"]
        );
        assert!(find_autolinks("xhttps://a.pl www. https://").is_empty());
        let quoted = find_autolinks("(https://a.pl/x)");
        assert_eq!(quoted[0].1, "https://a.pl/x");
    }
}
