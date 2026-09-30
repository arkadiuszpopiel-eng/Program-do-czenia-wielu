//! Ochrona fragmentów przed regułami: kod, URL-e, e-maile, domeny/pliki i słownik wymowy.

use voice_persona_contract::{CODE_ON_SCREEN, Lexicon};

use crate::numbers::digits;

/// Fragment tekstu przed tokenizacją.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Piece {
    /// Zwykły tekst — przechodzi przez reguły.
    Raw(String),
    /// Tekst gotowy do mowy — reguły go nie dotykają.
    Fixed(String),
}

const TLDS: &[&str] = &[
    "com", "pl", "org", "net", "io", "dev", "eu", "ai", "app", "gov", "edu", "info", "co", "uk",
    "de", "rs", "md", "txt", "toml", "json", "pdf", "py", "js", "ts", "html", "css", "csv", "png",
    "jpg", "zip", "exe", "yaml", "yml", "lock", "sh", "svelte", "wav",
];

fn map_raw(pieces: Vec<Piece>, f: impl Fn(&str) -> Vec<Piece>) -> Vec<Piece> {
    pieces
        .into_iter()
        .flat_map(|p| match p {
            Piece::Raw(s) => f(&s),
            fixed => vec![fixed],
        })
        .collect()
}

fn push_raw(out: &mut Vec<Piece>, s: &str) {
    if !s.is_empty() {
        out.push(Piece::Raw(s.to_owned()));
    }
}

/// Bloki ```…``` i kod w linii `…` → „(kod na ekranie)” (proste słowo w `…` zostaje słowem).
fn split_code(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        split_inline(&rest[..start], &mut out);
        out.push(Piece::Fixed(CODE_ON_SCREEN.to_owned()));
        let after = &rest[start + 3..];
        rest = match after.find("```") {
            Some(end) => &after[end + 3..],
            None => "",
        };
    }
    split_inline(rest, &mut out);
    out
}

fn split_inline(text: &str, out: &mut Vec<Piece>) {
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('`').filter(|e| !after[..*e].contains('\n')) else {
            break;
        };
        push_raw(out, &rest[..start]);
        let code = &after[..end];
        let simple = !code.is_empty()
            && code.chars().count() <= 32
            && code
                .chars()
                .all(|c| c.is_alphabetic() || c == '_' || c == '-');
        if simple {
            out.push(Piece::Raw(code.replace('_', " ")));
        } else {
            out.push(Piece::Fixed(CODE_ON_SCREEN.to_owned()));
        }
        rest = &after[end + 1..];
    }
    push_raw(out, rest);
}

/// Czyta etykietę domeny / fragment adresu: litery bez zmian, cyfry słownie, `-` → „myślnik”.
fn read_label(label: &str, lexicon: &Lexicon) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, words: &mut Vec<String>| {
        if cur.is_empty() {
            return;
        }
        let token = std::mem::take(cur);
        if token.chars().all(|c| c.is_ascii_digit()) {
            words.push(digits(&token));
        } else {
            let entry = lexicon
                .get(&token)
                .filter(|e| Lexicon::validate_entry(&e.word, &e.pron).is_ok());
            words.push(entry.map_or(token, |e| e.pron.clone()));
        }
    };
    let mut prev_digit: Option<bool> = None;
    for c in label.chars() {
        let spoken = match c {
            '-' => Some("myślnik"),
            '_' => Some("podkreślnik"),
            '.' => Some("kropka"),
            '+' => Some("plus"),
            _ => None,
        };
        if let Some(word) = spoken {
            flush(&mut cur, &mut words);
            words.push(word.to_owned());
            prev_digit = None;
            continue;
        }
        let is_digit = c.is_ascii_digit();
        if prev_digit.is_some_and(|d| d != is_digit) {
            flush(&mut cur, &mut words);
        }
        if c.is_alphanumeric() {
            cur.push(c);
            prev_digit = Some(is_digit);
        }
    }
    flush(&mut cur, &mut words);
    words.join(" ")
}

fn is_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && labels
            .last()
            .is_some_and(|tld| TLDS.contains(&tld.to_lowercase().as_str()))
        && labels
            .first()
            .is_some_and(|l| l.chars().any(|c| c.is_ascii_alphabetic()))
}

/// Czytanie linku, e-maila lub domeny/pliku (albo `None`, gdy to zwykłe słowo).
fn read_link(core: &str, lexicon: &Lexicon) -> Option<String> {
    let lower = core.to_lowercase();
    let scheme = ["https://", "http://", "ftp://", "www."]
        .iter()
        .find(|s| lower.starts_with(*s));
    if let Some(scheme) = scheme {
        let rest = core.get(scheme.len()..)?;
        let host_end = rest.find(['/', '?', '#', ':']).unwrap_or(rest.len());
        let host = rest[..host_end].trim_start_matches("www.");
        if host.is_empty() {
            return None;
        }
        return Some(format!("link do {}", read_label(host, lexicon)));
    }
    if let Some((local, domain)) = core.split_once('@') {
        let local_ok = !local.is_empty()
            && local
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | '%'));
        if local_ok && is_host(domain) {
            return Some(format!(
                "{} małpa {}",
                read_label(local, lexicon),
                read_label(domain, lexicon)
            ));
        }
        return None;
    }
    let (host, path) = match core.split_once('/') {
        Some((h, p)) => (h, Some(p)),
        None => (core, None),
    };
    if !is_host(host) {
        return None;
    }
    let spoken = read_label(host, lexicon);
    Some(if path.is_some() {
        format!("link do {spoken}")
    } else {
        spoken
    })
}

/// URL-e, e-maile i domeny w obrębie słów rozdzielonych białymi znakami.
fn split_links(text: &str, lexicon: &Lexicon) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut raw_start = 0;
    let mut idx = 0;
    for span in text.split_inclusive(char::is_whitespace) {
        let word = span.trim_end_matches(char::is_whitespace);
        let lead = word.len()
            - word
                .trim_start_matches(['(', '"', '\'', '„', '«', '<', '['])
                .len();
        let inner = &word[lead..];
        let core = inner.trim_end_matches([
            '.', ',', ';', ':', '!', '?', ')', '"', '\'', '”', '»', '>', ']',
        ]);
        if core.contains(['.', '@'])
            && let Some(spoken) = read_link(core, lexicon)
        {
            push_raw(&mut out, &text[raw_start..idx + lead]);
            out.push(Piece::Fixed(spoken));
            raw_start = idx + lead + core.len();
        }
        idx += span.len();
    }
    push_raw(&mut out, &text[raw_start..]);
    out
}

fn match_len(text: &str, key: &str) -> Option<usize> {
    let mut bytes = 0;
    let mut chars = text.chars();
    for k in key.chars() {
        let c = chars.next()?;
        if !c.to_lowercase().eq(k.to_lowercase()) {
            return None;
        }
        bytes += c.len_utf8();
    }
    Some(bytes)
}

/// Słownik wymowy: całe słowa, bez rozróżniania wielkości liter, najdłuższy klucz wygrywa.
fn apply_lexicon(text: &str, lexicon: &Lexicon) -> Vec<Piece> {
    let mut entries: Vec<(&str, &str)> = lexicon
        .iter()
        .filter(|(_, e)| Lexicon::validate_entry(&e.word, &e.pron).is_ok())
        .map(|(k, e)| (k, e.pron.as_str()))
        .collect();
    if entries.is_empty() {
        return vec![Piece::Raw(text.to_owned())];
    }
    entries.sort_by_key(|(k, _)| std::cmp::Reverse(k.chars().count()));
    let mut out = Vec::new();
    let mut raw_start = 0;
    let mut prev: Option<char> = None;
    let mut pos = 0;
    while pos < text.len() {
        let Some(c) = text[pos..].chars().next() else {
            break;
        };
        if !prev.is_some_and(char::is_alphanumeric) {
            let hit = entries.iter().find_map(|(key, pron)| {
                let len = match_len(&text[pos..], key)?;
                let after = text[pos + len..].chars().next();
                (!after.is_some_and(char::is_alphanumeric)).then_some((len, *pron))
            });
            if let Some((len, pron)) = hit {
                push_raw(&mut out, &text[raw_start..pos]);
                out.push(Piece::Fixed(pron.to_owned()));
                pos += len;
                raw_start = pos;
                prev = text[..pos].chars().last();
                continue;
            }
        }
        prev = Some(c);
        pos += c.len_utf8();
    }
    push_raw(&mut out, &text[raw_start..]);
    out
}

/// Pełna ochrona: kod → linki → słownik.
pub(crate) fn protect(text: &str, lexicon: &Lexicon) -> Vec<Piece> {
    let pieces = split_code(text);
    let pieces = map_raw(pieces, |s| split_links(s, lexicon));
    map_raw(pieces, |s| apply_lexicon(s, lexicon))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_code() {
        let lx = Lexicon::new();
        assert_eq!(
            read_link("https://github.com/x/y", &lx).as_deref(),
            Some("link do github kropka com")
        );
        assert_eq!(
            read_link("www.onet.pl", &lx).as_deref(),
            Some("link do onet kropka pl")
        );
        assert_eq!(
            read_link("jan.kowalski@firma.pl", &lx).as_deref(),
            Some("jan kropka kowalski małpa firma kropka pl")
        );
        assert_eq!(read_link("main.rs", &lx).as_deref(), Some("main kropka rs"));
        assert_eq!(read_link("m.in", &lx), None);
        assert_eq!(read_link("3.5", &lx), None);
        assert_eq!(
            read_link("site123.io", &lx).as_deref(),
            Some("site jeden dwa trzy kropka io")
        );
        let pieces = split_code("a ```x``` b `y` c `f(1)`");
        assert_eq!(
            pieces
                .iter()
                .filter(|p| matches!(p, Piece::Fixed(_)))
                .count(),
            2
        );
    }
}
