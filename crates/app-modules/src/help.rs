//! Wbudowana instrukcja (P5): przewodnik użytkownika `docs/user-guide/*.md` wkompilowany
//! w program, Markdown renderowany w Rust tym samym potokiem co odpowiedzi (`lib-markdown`:
//! pulldown-cmark + ammonia), wyszukiwanie bez wielkości liter i polskich znaków.

use app_api::dto::{HelpHit, HelpPage, HelpTopic};
use lib_quick::text::fold;

/// Działy w kolejności przewodnika.
const PAGES: [(&str, &str); 13] = [
    (
        "przewodnik",
        include_str!("../../../docs/user-guide/README.md"),
    ),
    (
        "pierwsze-uruchomienie",
        include_str!("../../../docs/user-guide/01-pierwsze-uruchomienie.md"),
    ),
    (
        "konta-i-model-lokalny",
        include_str!("../../../docs/user-guide/02-konta-i-model-lokalny.md"),
    ),
    (
        "rozmowa",
        include_str!("../../../docs/user-guide/03-rozmowa.md"),
    ),
    ("glos", include_str!("../../../docs/user-guide/04-glos.md")),
    (
        "agentki-i-zadania",
        include_str!("../../../docs/user-guide/05-agentki-i-zadania.md"),
    ),
    (
        "pamiec",
        include_str!("../../../docs/user-guide/06-pamiec.md"),
    ),
    (
        "bezpieczenstwo",
        include_str!("../../../docs/user-guide/07-bezpieczenstwo.md"),
    ),
    (
        "mosty-cli",
        include_str!("../../../docs/user-guide/08-mosty-cli.md"),
    ),
    (
        "dane-i-zdrowie",
        include_str!("../../../docs/user-guide/09-dane-i-zdrowie.md"),
    ),
    (
        "modele-i-silniki",
        include_str!("../../../docs/user-guide/10-modele-i-silniki.md"),
    ),
    (
        "kamery",
        include_str!("../../../docs/user-guide/12-kamery.md"),
    ),
    (
        "pierwszy-test-na-pc",
        include_str!("../../../docs/user-guide/11-pierwszy-test-na-pc.md"),
    ),
];

/// Najwięcej trafień wyszukiwania.
const MAX_HITS: usize = 30;
/// Długość fragmentu trafienia (znaki).
const SNIPPET: usize = 180;

fn title(md: &str) -> String {
    md.lines()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or("Instrukcja")
        .trim()
        .to_owned()
}

/// Spis działów.
pub fn topics() -> Vec<HelpTopic> {
    PAGES
        .iter()
        .map(|(id, md)| HelpTopic {
            id: (*id).to_owned(),
            title: title(md),
        })
        .collect()
}

/// Dział z treścią (HTML zsanitowany) albo `None` dla nieznanego identyfikatora.
pub fn page(id: &str) -> Option<HelpPage> {
    let (id, md) = PAGES.iter().find(|(p, _)| *p == id)?;
    Some(HelpPage {
        id: (*id).to_owned(),
        title: title(md),
        html_sanitized: lib_markdown::render(md),
    })
}

/// Tekst akapitu bez znaczników Markdown (do fragmentu trafienia).
fn plain(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .filter(|c| !matches!(c, '*' | '`' | '#' | '|' | '>'))
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn snippet(text: &str) -> String {
    let flat = plain(text);
    match flat.char_indices().nth(SNIPPET) {
        Some((cut, _)) => format!("{}…", &flat[..cut]),
        None => flat,
    }
}

/// Wyszukiwanie: akapity (bloki między pustymi wierszami) zawierające wszystkie słowa zapytania.
pub fn search(query: &str) -> Vec<HelpHit> {
    let words: Vec<String> = fold(query)
        .split_whitespace()
        .filter(|w| w.chars().count() >= 2)
        .map(str::to_owned)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (id, md) in PAGES {
        let page_title = title(md);
        let mut section = String::new();
        for block in md.split("\n\n") {
            let trimmed = block.trim();
            if let Some(h) = trimmed.lines().find_map(|l| l.strip_prefix("## ")) {
                section = h.trim().to_owned();
            }
            let folded = fold(trimmed);
            if !trimmed.is_empty() && words.iter().all(|w| folded.contains(w.as_str())) {
                hits.push(HelpHit {
                    id: id.to_owned(),
                    title: page_title.clone(),
                    section: section.clone(),
                    snippet: snippet(trimmed),
                });
                if hits.len() >= MAX_HITS {
                    return hits;
                }
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_renders_and_search_finds_text() {
        let list = topics();
        assert_eq!(list.len(), PAGES.len());
        let mut ids: Vec<&str> = list.iter().map(|t| t.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PAGES.len(), "unikalne identyfikatory");
        for t in &list {
            assert!(!t.title.is_empty() && t.title != "Instrukcja", "{}", t.id);
            let p = page(&t.id).unwrap();
            assert!(p.html_sanitized.contains("<h1"), "{}", t.id);
            assert!(!p.html_sanitized.contains("<script"), "{}", t.id);
        }
        assert!(page("nie-ma").is_none());
        let hits = search("Polecenia bez MODELU");
        assert!(hits.iter().any(|h| h.id == "rozmowa"), "{hits:?}");
        assert!(search("zolw zyrafa xqz").is_empty());
        assert!(search(" a ").is_empty(), "zbyt krótkie słowa");
        assert!(
            hits.iter()
                .all(|h| h.snippet.chars().count() <= SNIPPET + 1)
        );
    }
}
