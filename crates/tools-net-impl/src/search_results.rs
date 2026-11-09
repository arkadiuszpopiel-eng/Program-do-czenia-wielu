//! Projekcja niezaufanych źródeł bez ujawniania sekretów i bez nieograniczonego JSON.

use tools_common_contract::text;
use tools_net_contract::{SearchHit, SearchOut};

use crate::fetch::checked_target;

fn field(value: &str, remaining: &mut usize) -> (String, bool) {
    if *remaining == 0 {
        return (String::new(), !value.is_empty());
    }
    let redacted = text::redact_secrets(value);
    let mut chars = redacted.chars();
    let out: String = chars.by_ref().take(*remaining).collect();
    *remaining -= out.chars().count();
    (out, chars.next().is_some())
}

/// Ten sam budżet znaków obejmuje pola wyniku strukturalnego. Adresu nie skracamy ani
/// nie redagujemy do innego URL: niedozwolony, zawierający sekret lub za długi pomijamy.
pub(crate) fn project(hits: Vec<SearchHit>, max: u32, budget: usize) -> (SearchOut, bool) {
    let mut out = SearchOut {
        results: Vec::new(),
    };
    let mut remaining = budget;
    let mut limited = false;
    for hit in hits {
        if out.results.len() >= max as usize || remaining == 0 {
            limited = true;
            break;
        }
        let url_chars = hit.url.chars().count();
        if url_chars > remaining
            || hit.url.chars().any(char::is_control)
            || checked_target(&hit.url, "źródło wyszukiwania").is_err()
        {
            limited = true;
            continue;
        }
        remaining -= url_chars;
        let (title, title_cut) = field(&hit.title, &mut remaining);
        let (snippet, snippet_cut) = field(&hit.snippet, &mut remaining);
        limited |= title_cut || snippet_cut;
        out.results.push(SearchHit {
            title,
            url: hit.url,
            snippet,
        });
    }
    (out, limited)
}
