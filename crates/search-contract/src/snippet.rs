//! Fragment z podświetleniami (bez HTML — zakresy znaków; UI tylko wstawia tekst).

use lib_sqlstore::{search_tokens, tokenize};

use crate::types::{Highlight, Snippet};

/// Domyślna długość fragmentu w znakach (SPEC: `snippet_chars = 160`).
pub const DEFAULT_SNIPPET_CHARS: usize = 160;

const ELLIPSIS: char = '…';

/// Buduje fragment `text` wokół pierwszego trafienia `query` (słowa jak w FTS: bez diakrytyków,
/// prefiksy). Bez trafienia → początek tekstu. Znaki sterujące → spacje (fragment w jednej linii).
pub fn make_snippet(text: &str, query: &str, max_chars: usize) -> Snippet {
    let max_chars = max_chars.max(8);
    let terms = search_tokens(query);
    let matches: Vec<(usize, usize)> = tokenize(text)
        .into_iter()
        .filter(|t| terms.iter().any(|q| t.term.starts_with(q.as_str())))
        .map(|t| (t.start, t.end))
        .collect();
    let chars: Vec<char> = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let len = chars.len();
    let first = matches.first().map_or(0, |m| m.0);
    let mut start = first.saturating_sub(max_chars / 4);
    let end = (start + max_chars).min(len);
    start = end.saturating_sub(max_chars).min(start);
    let mut out = String::new();
    let lead = usize::from(start > 0);
    if start > 0 {
        out.push(ELLIPSIS);
    }
    out.extend(&chars[start..end]);
    if end < len {
        out.push(ELLIPSIS);
    }
    let highlights = matches
        .into_iter()
        .filter(|(s, e)| *s >= start && *e <= end)
        .map(|(s, e)| Highlight {
            start: s - start + lead,
            end: e - start + lead,
        })
        .collect();
    Snippet {
        text: out,
        highlights,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(s: &Snippet) -> Vec<String> {
        let chars: Vec<char> = s.text.chars().collect();
        s.highlights
            .iter()
            .map(|h| chars[h.start..h.end].iter().collect())
            .collect()
    }

    #[test]
    fn highlights_original_words_without_diacritics_in_query() {
        let s = make_snippet("Kolor żółć i gęś, znowu Żółć.", "zolc", 160);
        assert_eq!(s.text, "Kolor żółć i gęś, znowu Żółć.");
        assert_eq!(marked(&s), vec!["żółć", "Żółć"]);
    }

    #[test]
    fn prefix_matches_and_window_with_ellipsis() {
        let long = format!("{} łódka płynie {}", "a ".repeat(100), "b ".repeat(100));
        let s = make_snippet(&long, "łód", 40);
        assert!(s.text.starts_with('…') && s.text.ends_with('…'));
        assert!(s.text.chars().count() <= 42);
        assert_eq!(marked(&s), vec!["łódka"]);
    }

    #[test]
    fn no_match_gives_head_and_controls_become_spaces() {
        let s = make_snippet("linia 1\nlinia 2", "xyz", 160);
        assert_eq!(s.text, "linia 1 linia 2");
        assert!(s.highlights.is_empty());
    }
}
