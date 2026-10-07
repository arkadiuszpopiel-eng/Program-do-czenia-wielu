//! Reranking wyników `recall` (port [`Reranker`]; domyślnie [`HeuristicReranker`]) i lekka
//! normalizacja polskich słów (rdzenie bez końcówek fleksyjnych, słowa funkcyjne).
//!
//! Wyszukiwanie hybrydowe (`search`: FTS5 + wektor + RRF) daje kandydatów; reranker ustala
//! kolejność końcową z cech: pokrycie rdzeni zapytania, wynik wyszukiwania, pewność, przypięcie,
//! zaufanie, świeżość. Model rerankingu (cross-encoder) można podpiąć tym samym portem.

use chrono::{DateTime, Utc};
use lib_sqlstore::search_tokens;

use crate::types::{Layer, MemoryEntry, Provenance};

/// Słowa funkcyjne (forma złożona `fold_pl`) pomijane przy dopasowaniu treści.
const STOPWORDS: &[&str] = &[
    "a",
    "aby",
    "ale",
    "albo",
    "bez",
    "bo",
    "by",
    "byc",
    "byl",
    "byla",
    "bylo",
    "co",
    "czy",
    "czego",
    "czym",
    "dla",
    "do",
    "gdzie",
    "go",
    "i",
    "ich",
    "ile",
    "im",
    "ja",
    "jak",
    "jaka",
    "jaki",
    "jakie",
    "jakiego",
    "jakiej",
    "jakim",
    "jakimi",
    "jakich",
    "jest",
    "jestem",
    "jej",
    "jego",
    "juz",
    "kiedy",
    "kto",
    "ktora",
    "ktore",
    "ktory",
    "ktorej",
    "ktorego",
    "ktorym",
    "ma",
    "mam",
    "mi",
    "mnie",
    "moj",
    "moja",
    "moje",
    "mojej",
    "mojego",
    "moim",
    "moich",
    "mu",
    "na",
    "nad",
    "nam",
    "nas",
    "nie",
    "o",
    "od",
    "on",
    "ona",
    "oni",
    "oraz",
    "po",
    "pod",
    "przez",
    "przy",
    "sa",
    "se",
    "sie",
    "so",
    "ta",
    "tak",
    "te",
    "tego",
    "tej",
    "ten",
    "to",
    "tu",
    "ty",
    "tym",
    "u",
    "w",
    "we",
    "wiesz",
    "z",
    "za",
    "ze",
    "zna",
    "znasz",
    "uzytkownik",
    "uzytkownika",
    "uzytkownikowi",
    "uzytkownikiem",
    "pamietasz",
    "przypomnij",
    "powiedz",
];

/// Końcówki fleksyjne (forma złożona), od najdłuższych; rdzeń ma co najmniej 3 znaki.
const SUFFIXES: &[&str] = &[
    "owaniami", "owaniach", "owania", "owanie", "owaniu", "ującymi", "iami", "ach", "ami", "ego",
    "emu", "ich", "ych", "imi", "ymi", "owi", "iem", "iej", "ow", "om", "em", "ej", "ie", "ia",
    "iu", "io", "ii", "ym", "im", "a", "e", "i", "o", "u", "y",
];

/// Czy słowo (forma złożona) jest funkcyjne.
pub fn is_stopword(term: &str) -> bool {
    STOPWORDS.contains(&term)
}

/// Oboczności miejscownika (`-cie`, `-dzie`, `-rze`, `-le`) → spółgłoska tematu
/// („herbacie” → „herbat”, „ogrodzie” → „ogrod”, „rowerze” → „rower”, „stole” → „stol”).
const ALTERNATIONS: &[(&str, &str)] = &[("dzie", "d"), ("cie", "t"), ("rze", "r"), ("le", "l")];

/// Rdzeń słowa: forma złożona bez jednej końcówki fleksyjnej (min. 3 znaki rdzenia).
pub fn stem_pl(term: &str) -> String {
    let chars = term.chars().count();
    for (ending, consonant) in ALTERNATIONS {
        let len = ending.chars().count();
        if chars >= len + 3
            && let Some(base) = term.strip_suffix(ending)
        {
            return format!("{base}{consonant}");
        }
    }
    for suffix in SUFFIXES {
        let len = suffix.chars().count();
        if chars >= len + 3 && term.ends_with(suffix) {
            return term.chars().take(chars - len).collect();
        }
    }
    term.to_owned()
}

/// Rdzenie słów treściowych tekstu (bez słów funkcyjnych i jednoliterowych), bez duplikatów,
/// w kolejności wystąpienia.
pub fn content_stems(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for term in search_tokens(text) {
        if term.chars().count() < 2 || is_stopword(&term) {
            continue;
        }
        let stem = stem_pl(&term);
        if !out.contains(&stem) {
            out.push(stem);
        }
    }
    out
}

/// Czy rdzenie pasują (jeden jest prefiksem drugiego; krótszy ≥ 3 znaki, liczby — dokładnie).
pub fn stems_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let numeric = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if numeric(a) || numeric(b) {
        return false;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.chars().count() >= 3 && long.starts_with(short)
}

/// Pokrycie rdzeni zapytania przez rdzenie dokumentu (0–1).
pub fn stem_coverage(query: &[String], doc: &[String]) -> f32 {
    if query.is_empty() {
        return 0.0;
    }
    let hits = query
        .iter()
        .filter(|q| doc.iter().any(|d| stems_match(q, d)))
        .count();
    ratio(hits, query.len())
}

fn ratio(a: usize, b: usize) -> f32 {
    let a = f32::from(u16::try_from(a).unwrap_or(u16::MAX));
    let b = f32::from(u16::try_from(b.max(1)).unwrap_or(u16::MAX));
    a / b
}

/// Kandydat do rerankingu.
#[derive(Debug, Clone, Copy)]
pub struct RerankItem<'a> {
    /// Wpis.
    pub entry: &'a MemoryEntry,
    /// Wynik wyszukiwania hybrydowego (RRF; skala zależna od magazynu).
    pub retrieval: f32,
}

/// Port rerankingu: zwraca wynik dla każdego kandydata (ta sama kolejność co wejście).
pub trait Reranker: Send + Sync {
    /// Nazwa (raport ewaluacji).
    fn name(&self) -> &str;
    /// Wyniki (większy = lepszy).
    fn rerank(&self, query: &str, items: &[RerankItem<'_>], now: DateTime<Utc>) -> Vec<f32>;
}

/// Domyślny reranker bez modelu: `0,55·rdzenie + 0,30·wyszukiwanie + 0,05·pewność` + drobne
/// premie (przypięcie, fakt semantyczny, użytkownik, świeżość) i kara za treść niezaufaną.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicReranker;

impl HeuristicReranker {
    fn doc_stems(entry: &MemoryEntry) -> Vec<String> {
        let mut text = entry.text.clone();
        for extra in entry.subject.iter().chain(entry.entities.iter()) {
            text.push(' ');
            text.push_str(extra);
        }
        content_stems(&text)
    }
}

impl Reranker for HeuristicReranker {
    fn name(&self) -> &str {
        "heuristic-v1"
    }

    fn rerank(&self, query: &str, items: &[RerankItem<'_>], now: DateTime<Utc>) -> Vec<f32> {
        let stems = content_stems(query);
        let max_retrieval = items
            .iter()
            .map(|i| i.retrieval)
            .fold(0.0_f32, f32::max)
            .max(f32::EPSILON);
        items
            .iter()
            .map(|item| {
                let e = item.entry;
                let lexical = stem_coverage(&stems, &Self::doc_stems(e));
                let retrieval = (item.retrieval / max_retrieval).clamp(0.0, 1.0);
                let age_days = (now - e.created_at).num_days().max(0);
                let fresh = 0.02 / (1.0 + ratio(usize::try_from(age_days).unwrap_or(0), 365));
                let mut score = 0.55 * lexical + 0.30 * retrieval + 0.05 * e.confidence + fresh;
                if e.pinned {
                    score += 0.04;
                }
                if e.layer == Layer::Semantic {
                    score += 0.02;
                }
                if e.provenance == Provenance::User {
                    score += 0.01;
                }
                if !e.trusted {
                    score -= 0.05;
                }
                score
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_handle_polish_inflection() {
        assert_eq!(stem_pl("kawe"), "kaw");
        assert_eq!(stem_pl("kawa"), "kaw");
        assert_eq!(stem_pl("kawy"), "kaw");
        assert_eq!(stem_pl("psem"), "psem");
        assert_eq!(stem_pl("herbacie"), stem_pl("herbata"));
        assert_eq!(stem_pl("ogrodzie"), stem_pl("ogrod"));
        assert_eq!(stem_pl("rowerze"), stem_pl("rower"));
        assert_eq!(
            content_stems("Jaką kawę pije użytkownik?"),
            vec!["kaw", "pij"]
        );
        assert!(stems_match("kaw", "kawiarni") && !stems_match("ka", "kawa"));
        assert!(!stems_match("43", "430") && stems_match("43", "43"));
        let q = content_stems("ulubiony kolor");
        let d = content_stems("Ulubionym kolorem jest żółty");
        assert!((stem_coverage(&q, &d) - 1.0).abs() < f32::EPSILON);
        assert_eq!(stem_coverage(&[], &d), 0.0);
    }
}
