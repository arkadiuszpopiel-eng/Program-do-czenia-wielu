//! Testy property-based normalizacji tekstu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_sqlstore::{fold_char, fold_pl, fts5_match, search_tokens, tokenize};
use proptest::prelude::*;

proptest! {
    /// Liczba znaków się nie zmienia (pozycje podświetleń pozostają ważne).
    #[test]
    fn fold_preserves_char_count(s in "\\PC{0,64}") {
        prop_assert_eq!(fold_pl(&s).chars().count(), s.chars().count());
    }

    /// Składanie jest idempotentne.
    #[test]
    fn fold_is_idempotent(s in "\\PC{0,64}") {
        prop_assert_eq!(fold_pl(&fold_pl(&s)), fold_pl(&s));
    }

    /// Polskie litery zawsze składają się do ASCII.
    #[test]
    fn polish_text_folds_to_ascii(s in "[a-zA-ZąćęłńóśźżĄĆĘŁŃÓŚŹŻ ]{0,48}") {
        prop_assert!(fold_pl(&s).is_ascii());
        prop_assert_eq!(search_tokens(&s), search_tokens(&fold_pl(&s)));
    }

    /// Zakresy słów są rosnące, rozłączne i mieszczą się w tekście.
    #[test]
    fn token_spans_are_ordered(s in "\\PC{0,64}") {
        let len = s.chars().count();
        let mut last = 0;
        for t in tokenize(&s) {
            prop_assert!(t.start >= last && t.start < t.end && t.end <= len);
            prop_assert!(!t.term.is_empty());
            last = t.end;
        }
    }

    /// Wyrażenie MATCH zawiera tylko cytowane frazy z prefiksem.
    #[test]
    fn match_expression_is_quoted(s in "\\PC{0,64}") {
        if let Some(expr) = fts5_match(&s) {
            for part in expr.split(' ') {
                prop_assert!(part.starts_with('"') && part.ends_with("\"*"));
            }
        }
    }

    /// Każdy znak ASCII przechodzi bez zmian.
    #[test]
    fn ascii_unchanged(c in proptest::char::range('\u{0}', '\u{7f}')) {
        prop_assert_eq!(fold_char(c), c);
    }
}
