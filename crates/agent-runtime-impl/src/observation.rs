//! Skracanie obserwacji zachowuje podsumowanie i końcową diagnostykę kompilatora.

const OMITTED: &str = "\n[… pominięto środek wyjścia …]\n";

pub(crate) fn bounded_output(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_owned();
    }
    let marker_chars = OMITTED.chars().count();
    if max_chars <= marker_chars {
        return text.chars().take(max_chars).collect();
    }
    let available = max_chars - marker_chars;
    let head_chars = available.div_ceil(2);
    let tail_chars = available - head_chars;
    let mut body: String = text.chars().take(head_chars).collect();
    body.push_str(OMITTED);
    body.extend(text.chars().skip(count - tail_chars));
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn keeps_summary_and_final_error_within_existing_budget() {
        let output = format!(
            "Kompilacja: błąd\n{}\nerror[E0308]: zły typ",
            "x".repeat(30_000)
        );
        let body = bounded_output(&output, 24_000);
        assert_eq!(body.chars().count(), 24_000);
        assert!(body.starts_with("Kompilacja: błąd\n"));
        assert!(body.contains(OMITTED));
        assert!(body.ends_with("error[E0308]: zły typ"));
    }

    #[test]
    fn short_output_is_unchanged_including_unicode() {
        let output = "Zażółć 🦀\nwynik";
        assert_eq!(bounded_output(output, output.chars().count()), output);
        assert_eq!(bounded_output(output, usize::MAX), output);
        assert_eq!(bounded_output(output, 0), "");
    }

    proptest! {
        #[test]
        fn arbitrary_unicode_and_budgets_are_bounded(text in ".{0,1000}", budget in 0usize..1100) {
            let body = bounded_output(&text, budget);
            prop_assert_eq!(body.chars().count(), text.chars().count().min(budget));
            if text.chars().count() <= budget {
                prop_assert_eq!(body, text);
            } else if budget > OMITTED.chars().count() + 1 {
                prop_assert_eq!(body.chars().next(), text.chars().next());
                prop_assert_eq!(body.chars().last(), text.chars().last());
                prop_assert!(body.contains(OMITTED));
            }
        }
    }
}
