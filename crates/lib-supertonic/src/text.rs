//! Przygotowanie tekstu dla Supertonic 3 (na podstawie przykładu producenta, licencja MIT):
//! normalizacja NFKD, usunięcie emoji i znaków specjalnych, porządek w spacjach i interpunkcji,
//! kropka na końcu, znaczniki języka `<pl>…</pl>`, zamiana znaków na indeksy modelu.

use unicode_normalization::UnicodeNormalization;

/// Języki modelu (31 + `na` = bez wskazania).
pub const LANGS: &[&str] = &[
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi", "fr", "hi", "hr", "hu",
    "id", "it", "lt", "lv", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "tr", "uk", "vi", "na",
];

/// Czy model zna język.
pub fn is_lang(lang: &str) -> bool {
    LANGS.contains(&lang)
}

fn is_emoji(c: char) -> bool {
    matches!(
        u32::from(c),
        0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x1F1E6..=0x1F1FF
    )
}

const REPLACE: &[(&str, &str)] = &[
    ("–", "-"),
    ("‑", "-"),
    ("—", "-"),
    ("_", " "),
    ("\u{201C}", "\""),
    ("\u{201D}", "\""),
    ("\u{201E}", "\""),
    ("\u{2018}", "'"),
    ("\u{2019}", "'"),
    ("´", "'"),
    ("`", "'"),
    ("[", " "),
    ("]", " "),
    ("|", " "),
    ("/", " "),
    ("#", " "),
    ("→", " "),
    ("←", " "),
    ("♥", ""),
    ("☆", ""),
    ("♡", ""),
    ("©", ""),
    ("\\", ""),
    ("@", " at "),
];

const END_PUNCT: &[char] = &[
    '.', '!', '?', ';', ':', ',', '\'', '"', ')', ']', '}', '…', '。', '」', '』', '】', '〉',
    '》', '›', '»',
];

/// Tekst gotowy dla modelu: oczyszczony i owinięty znacznikami języka.
pub fn prepare(text: &str, lang: &str) -> Option<String> {
    if !is_lang(lang) {
        return None;
    }
    let mut t: String = text.nfkd().filter(|c| !is_emoji(*c)).collect();
    for (from, to) in REPLACE {
        if t.contains(from) {
            t = t.replace(from, to);
        }
    }
    let mut t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    for p in [",", ".", "!", "?", ";", ":", "'"] {
        let spaced = format!(" {p}");
        while t.contains(&spaced) {
            t = t.replace(&spaced, p);
        }
    }
    for dup in ["\"\"", "''"] {
        while t.contains(dup) {
            t = t.replace(dup, &dup[..1]);
        }
    }
    let mut t = t.trim().to_owned();
    if t.is_empty() {
        return None;
    }
    if !t.ends_with(END_PUNCT) {
        t.push('.');
    }
    Some(format!("<{lang}>{t}</{lang}>"))
}

/// Indeksy znaków według tabeli modelu (`unicode_indexer.json`); znak spoza tabeli = −1.
pub fn to_ids(prepared: &str, indexer: &[i64]) -> Vec<i64> {
    prepared
        .chars()
        .map(|c| indexer.get(c as usize).copied().unwrap_or(-1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_with_language_and_adds_period() {
        assert_eq!(
            prepare("Dzień dobry", "pl").as_deref().map(str::len),
            // NFKD rozkłada „ń” na dwa znaki — długość w bajtach rośnie, sens zostaje.
            Some("<pl>Dzien\u{301} dobry.</pl>".len())
        );
        assert_eq!(prepare("Tak?", "pl").as_deref(), Some("<pl>Tak?</pl>"));
    }

    #[test]
    fn cleans_spacing_symbols_and_emoji() {
        assert_eq!(
            prepare("To  jest —  test , prawda ? 😀", "pl").as_deref(),
            Some("<pl>To jest - test, prawda?</pl>")
        );
        assert_eq!(
            prepare("a_b [c] #d", "en").as_deref(),
            Some("<en>a b c d.</en>")
        );
    }

    #[test]
    fn rejects_unknown_language_and_empty_text() {
        assert_eq!(prepare("hej", "xx"), None);
        assert_eq!(prepare("  😀 ", "pl"), None);
    }

    #[test]
    fn maps_characters_through_indexer() {
        let mut indexer = vec![-1i64; 128];
        indexer['a' as usize] = 7;
        indexer['<' as usize] = 1;
        assert_eq!(to_ids("<aż", &indexer), vec![1, 7, -1]);
    }
}
