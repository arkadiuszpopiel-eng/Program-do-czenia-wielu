//! Normalizacja tekstu wypowiedzi (PL): małe litery, zdjęte znaki diakrytyczne, podział na słowa.
//! Transkrypcja STT i tekst pisany bez polskich znaków („przejmij weryfikacje”) dają to samo.

/// Zamienia na małe litery i zdejmuje polskie znaki diakrytyczne (ą→a, ł→l, ż/ź→z…).
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            other => other,
        })
        .collect()
}

/// Słowo wypowiedzi po normalizacji z kontekstem interpunkcji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Słowo po `fold`.
    pub word: String,
    /// Poprzedzone znakiem `@` (adresowanie w composerze).
    pub at: bool,
    /// Bezpośrednio przed słowem był przecinek (lub inny znak oddzielający wtrącenie).
    pub comma_before: bool,
    /// Bezpośrednio po słowie jest przecinek.
    pub comma_after: bool,
    /// Po słowie kończy się zdanie (`.`, `!`, `?`) albo tekst.
    pub ends_sentence: bool,
}

/// Dzieli tekst na słowa (litery i cyfry) z informacją o interpunkcji wokół.
pub fn tokenize(text: &str) -> Vec<Token> {
    let folded = fold(text);
    let mut tokens: Vec<Token> = Vec::new();
    let mut word = String::new();
    let mut pending_at = false;
    let mut pending_comma = false;
    fn flush(word: &mut String, tokens: &mut Vec<Token>, at: &mut bool, comma: &mut bool) {
        if !word.is_empty() {
            tokens.push(Token {
                word: std::mem::take(word),
                at: std::mem::take(at),
                comma_before: std::mem::take(comma),
                comma_after: false,
                ends_sentence: false,
            });
        }
    }
    for c in folded.chars() {
        if c.is_alphanumeric() {
            word.push(c);
            continue;
        }
        flush(&mut word, &mut tokens, &mut pending_at, &mut pending_comma);
        match c {
            '@' => pending_at = true,
            ',' | ';' | '—' | '–' | ':' => {
                pending_comma = true;
                if let Some(last) = tokens.last_mut() {
                    last.comma_after = true;
                }
            }
            '.' | '!' | '?' | '…' => {
                if let Some(last) = tokens.last_mut() {
                    last.ends_sentence = true;
                }
            }
            _ => {}
        }
    }
    flush(&mut word, &mut tokens, &mut pending_at, &mut pending_comma);
    if let Some(last) = tokens.last_mut() {
        last.ends_sentence = true;
    }
    tokens
}

/// Czy wypowiedź jest pytaniem (znak zapytania na końcu).
pub fn is_question(text: &str) -> bool {
    text.trim_end().ends_with('?')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_polish() {
        assert_eq!(
            fold("Zażółć GĘŚLĄ Jaźń, Delcie"),
            "zazolc gesla jazn, delcie"
        );
    }

    #[test]
    fn tokens_with_punctuation() {
        let t = tokenize("@Beta, teraz ty prowadzisz. Hej Gamo!");
        let words: Vec<&str> = t.iter().map(|t| t.word.as_str()).collect();
        assert_eq!(words, ["beta", "teraz", "ty", "prowadzisz", "hej", "gamo"]);
        assert!(t[0].at && t[0].comma_after && !t[0].comma_before);
        assert!(t[1].comma_before);
        assert!(t[3].ends_sentence && t[5].ends_sentence);
        assert!(tokenize("").is_empty());
        assert!(is_question("Kto prowadzi? "));
    }
}
