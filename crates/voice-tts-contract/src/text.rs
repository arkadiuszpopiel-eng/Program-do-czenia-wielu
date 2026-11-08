//! Chunker (zdanie po zdaniu, polskie skróty) i estymacja znaczników słów z długości słów.

use crate::WordMark;

/// Skróty, po których kropka nie kończy zdania (małymi literami, bez kropki).
const ABBREVIATIONS: [&str; 24] = [
    "np", "itd", "itp", "m.in", "dr", "prof", "ok", "tzw", "tj", "ul", "godz", "r", "w", "pkt",
    "nr", "str", "mgr", "inż", "hab", "al", "pl", "tzn", "wg", "zob",
];
/// Zdania dłuższe niż tyle znaków dzielimy na przecinkach/średnikach (niższe TTFB).
const MAX_CHUNK_CHARS: usize = 180;

fn ends_sentence(word: &str, next: Option<&str>) -> bool {
    let Some(last) = word.chars().last() else {
        return false;
    };
    if matches!(last, '!' | '?' | '…') {
        return true;
    }
    if last != '.' {
        return false;
    }
    let stem = word.trim_end_matches('.').to_lowercase();
    if ABBREVIATIONS.contains(&stem.as_str()) {
        return false;
    }
    // „3. maja”, „1. punkt” — liczba porządkowa przed małą literą.
    let next_lower = next
        .and_then(|n| n.chars().next())
        .is_some_and(char::is_lowercase);
    !(stem.chars().all(|c| c.is_ascii_digit()) && next_lower)
}

/// Dzieli tekst na fragmenty do syntezy (zdania; zbyt długie — na przecinkach).
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let mut current: Vec<&str> = Vec::new();
        let mut len = 0;
        for (i, w) in words.iter().enumerate() {
            current.push(w);
            len += w.len() + 1;
            let soft =
                len > MAX_CHUNK_CHARS && (w.ends_with(',') || w.ends_with(';') || w.ends_with(':'));
            if ends_sentence(w, words.get(i + 1).copied()) || soft {
                out.push(current.join(" "));
                current.clear();
                len = 0;
            }
        }
        if !current.is_empty() {
            out.push(current.join(" "));
        }
    }
    out
}

/// Waga słowa w czasie mówienia: znaki + przerwa po interpunkcji.
fn weight(word: &str) -> f32 {
    let pause = match word.chars().last() {
        Some('.' | '!' | '?' | '…') => 4.0,
        Some(',' | ';' | ':') => 2.0,
        _ => 0.0,
    };
    word.chars().filter(|c| c.is_alphanumeric()).count().max(1) as f32 + 1.0 + pause
}

/// Estymuje znaczniki słów fragmentu: czas `duration_ms` rozdzielony proporcjonalnie do długości
/// słów; `first_idx` — indeks pierwszego słowa w całej wypowiedzi, `offset_ms` — początek fragmentu.
pub fn estimate_marks(
    words: &[&str],
    first_idx: u32,
    offset_ms: u32,
    duration_ms: u32,
) -> Vec<WordMark> {
    let total: f32 = words.iter().map(|w| weight(w)).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut t = 0.0f32;
    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let wt = weight(w);
            let chars = w.chars().filter(|c| c.is_alphanumeric()).count().max(1) as f32 + 1.0;
            let start = offset_ms as f32 + t / total * duration_ms as f32;
            let end = start + chars / total * duration_ms as f32;
            t += wt;
            WordMark {
                word_idx: first_idx + i as u32,
                word: (*w).to_owned(),
                start_ms: start.round() as u32,
                end_ms: end.round() as u32,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_polish_sentences_with_abbreviations() {
        let t = "Dzień dobry! Mam np. trzy pliki, m.in. raport. Spotkanie 3. maja o godz. 10. Co dalej?\nNowa linia";
        assert_eq!(
            split_sentences(t),
            vec![
                "Dzień dobry!",
                "Mam np. trzy pliki, m.in. raport.",
                "Spotkanie 3. maja o godz. 10.",
                "Co dalej?",
                "Nowa linia",
            ]
        );
        assert!(split_sentences("   ").is_empty());
        let long = "a, ".repeat(100);
        assert!(
            split_sentences(&long).len() >= 2,
            "długie zdanie dzielone na przecinkach"
        );
    }

    #[test]
    fn marks_are_ordered_and_cover_duration() {
        let words = ["Dzień", "dobry,", "Alfa."];
        let m = estimate_marks(&words, 4, 1_000, 1_500);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].word_idx, 4);
        assert_eq!(m[0].start_ms, 1_000);
        assert!(m.windows(2).all(|w| w[0].end_ms <= w[1].start_ms));
        assert!(m[2].end_ms <= 2_500);
        assert!(estimate_marks(&[], 0, 0, 100).is_empty());
    }
}
