//! Hezytacje i zakończenia w transkrypcie częściowym (PL + podstawowe EN).

/// Rodzaj hezytacji na końcu transkryptu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hesitation {
    /// Wypełniacz („yyy”, „eee”, „znaczy”, „hmm”).
    Filler,
    /// Niedokończona konstrukcja („i…”, „że”, przyimek, przecinek, wielokropek).
    Unfinished,
}

const FILLERS: &[&str] = &[
    "y", "yy", "yyy", "yyyy", "e", "ee", "eee", "eeee", "eem", "eeem", "hmm", "hm", "mhm", "yhm",
    "mmm", "znaczy", "jakby", "wiesz", "tego", "ten", "um", "uh", "uhm", "erm",
];
const UNFINISHED: &[&str] = &[
    "i", "oraz", "a", "ale", "lub", "albo", "czy", "że", "żeby", "bo", "więc", "który", "która",
    "które", "którego", "której", "jeśli", "jeżeli", "gdy", "kiedy", "gdzie", "jak", "aby", "w",
    "we", "na", "z", "ze", "do", "od", "o", "po", "przy", "dla", "za", "pod", "nad", "przed",
    "między", "bez", "u", "and", "or", "but", "the", "so", "because", "with", "of",
];

/// Ostatnie słowo (małe litery, bez interpunkcji).
fn last_word(text: &str) -> Option<String> {
    let word = text.split_whitespace().last()?;
    let clean: String = word
        .chars()
        .filter(|c| c.is_alphabetic())
        .flat_map(char::to_lowercase)
        .collect();
    (!clean.is_empty()).then_some(clean)
}

/// Hezytacja na końcu transkryptu, jeśli jest.
pub fn hesitation(text: &str) -> Option<Hesitation> {
    let t = text.trim_end();
    if t.is_empty() {
        return None;
    }
    if t.ends_with("...")
        || t.ends_with('…')
        || t.ends_with(',')
        || t.ends_with('-')
        || t.ends_with('–')
    {
        return Some(Hesitation::Unfinished);
    }
    if ends_clearly(t) {
        return None;
    }
    let word = last_word(t)?;
    if FILLERS.contains(&word.as_str()) {
        Some(Hesitation::Filler)
    } else if UNFINISHED.contains(&word.as_str()) {
        Some(Hesitation::Unfinished)
    } else {
        None
    }
}

/// Czy transkrypt kończy się wyraźnym końcem zdania (`.`, `?`, `!`, bez wielokropka).
pub fn ends_clearly(text: &str) -> bool {
    let t = text.trim_end();
    (t.ends_with('.') || t.ends_with('?') || t.ends_with('!')) && !t.ends_with("...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_hesitations() {
        assert_eq!(hesitation("Chciałbym, żeby yyy"), Some(Hesitation::Filler));
        assert_eq!(hesitation("to znaczy"), Some(Hesitation::Filler));
        assert_eq!(hesitation("kupiłam mleko i"), Some(Hesitation::Unfinished));
        assert_eq!(hesitation("myślę, że…"), Some(Hesitation::Unfinished));
        assert_eq!(hesitation("pojedziemy do"), Some(Hesitation::Unfinished));
        assert_eq!(hesitation("Jaka jest pogoda?"), None);
        assert_eq!(hesitation("zrób to"), None);
        assert_eq!(hesitation(""), None);
        assert!(ends_clearly("Koniec.") && !ends_clearly("Koniec...") && !ends_clearly("koniec"));
    }
}
