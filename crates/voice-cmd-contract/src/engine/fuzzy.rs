//! Tolerancja szumu ASR: dopasowanie słów z odległością edycyjną.

/// Pewność dopasowania dokładnego.
pub const EXACT: f32 = 1.0;
/// Pewność dopasowania z jedną edycją (słowa ≥ 5 znaków).
pub const ONE_EDIT: f32 = 0.85;
/// Pewność dopasowania z dwiema edycjami (słowa ≥ 8 znaków).
pub const TWO_EDITS: f32 = 0.7;

/// Odległość Levenshteina na znakach.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Pewność, że słowo `heard` (po `fold`) to `expected` (po `fold`); `None` = brak dopasowania.
pub fn word_score(heard: &str, expected: &str) -> Option<f32> {
    if heard == expected {
        return Some(EXACT);
    }
    let len = expected.chars().count();
    if len < 5 || heard.chars().count().abs_diff(len) > 2 {
        return None;
    }
    match levenshtein(heard, expected) {
        1 => Some(ONE_EDIT),
        2 if len >= 8 => Some(TWO_EDITS),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_and_scores() {
        assert_eq!(levenshtein("kontynuuj", "kontynuluj"), 1);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(word_score("stop", "stop"), Some(EXACT));
        assert_eq!(word_score("stok", "stop"), None);
        assert_eq!(word_score("palza", "pauza"), Some(ONE_EDIT));
        assert_eq!(word_score("kontynulujj", "kontynuuj"), Some(TWO_EDITS));
        assert_eq!(word_score("kot", "kontynuuj"), None);
    }
}
