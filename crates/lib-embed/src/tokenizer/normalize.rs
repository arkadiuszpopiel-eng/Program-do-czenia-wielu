//! Normalizatory i pre-tokenizery `tokenizer.json` (podzbiór HF `tokenizers` potrzebny modelom
//! SentencePiece/XLM-R): `Precompiled`, `Replace`, `Strip`, `NFC/NFD/NFKC/NFKD`, `Lowercase`,
//! `Sequence`; `Metaspace`, `WhitespaceSplit`, `Sequence`. Nieznany typ → błąd ładowania (nigdy
//! cicha, niezgodna tokenizacja).

use regex::Regex;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use super::precompiled::Precompiled;
use crate::error::EmbedError;

fn bad(reason: impl Into<String>) -> EmbedError {
    EmbedError::Tokenizer(reason.into())
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn kind(v: &Value) -> Result<&str, EmbedError> {
    str_field(v, "type").ok_or_else(|| bad("element bez pola `type`"))
}

/// Normalizator.
#[derive(Debug, Clone)]
pub enum Normalizer {
    /// Reguły SentencePiece.
    Precompiled(Box<Precompiled>),
    /// Zamiana wzorca (wyrażenie regularne albo dosłowny napis).
    Replace(Regex, String),
    /// Usunięcie białych znaków z lewej/prawej.
    Strip(bool, bool),
    /// Normalizacja Unicode.
    Nfc,
    /// Normalizacja Unicode.
    Nfd,
    /// Normalizacja Unicode.
    Nfkc,
    /// Normalizacja Unicode.
    Nfkd,
    /// Małe litery.
    Lowercase,
}

impl Normalizer {
    /// Parsuje element `normalizer` (`null` → pusta lista).
    pub fn parse(v: &Value) -> Result<Vec<Normalizer>, EmbedError> {
        if v.is_null() {
            return Ok(Vec::new());
        }
        let one = match kind(v)? {
            "Sequence" => {
                let items = v
                    .get("normalizers")
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad("Sequence bez `normalizers`"))?;
                let mut out = Vec::new();
                for item in items {
                    out.extend(Self::parse(item)?);
                }
                return Ok(out);
            }
            "Precompiled" => {
                let map = str_field(v, "precompiled_charsmap").unwrap_or_default();
                if map.is_empty() {
                    return Ok(Vec::new());
                }
                Normalizer::Precompiled(Box::new(Precompiled::from_base64(map)?))
            }
            "Replace" => {
                let pattern = v
                    .get("pattern")
                    .ok_or_else(|| bad("Replace bez `pattern`"))?;
                let regex = match (str_field(pattern, "Regex"), str_field(pattern, "String")) {
                    (Some(r), _) => r.to_owned(),
                    (None, Some(s)) => regex::escape(s),
                    _ => return Err(bad("Replace: nieznany wzorzec")),
                };
                let content = str_field(v, "content").unwrap_or_default().to_owned();
                let regex = Regex::new(&regex).map_err(|e| bad(format!("Replace: {e}")))?;
                Normalizer::Replace(regex, content)
            }
            "Strip" => Normalizer::Strip(
                v.get("strip_left")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                v.get("strip_right")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
            "NFC" => Normalizer::Nfc,
            "NFD" => Normalizer::Nfd,
            "NFKC" => Normalizer::Nfkc,
            "NFKD" => Normalizer::Nfkd,
            "Lowercase" => Normalizer::Lowercase,
            other => return Err(bad(format!("nieobsługiwany normalizator `{other}`"))),
        };
        Ok(vec![one])
    }

    /// Stosuje normalizator.
    pub fn apply(&self, text: &str) -> String {
        match self {
            Normalizer::Precompiled(p) => p.normalize(text),
            Normalizer::Replace(re, content) => re.replace_all(text, content.as_str()).into_owned(),
            Normalizer::Strip(left, right) => {
                let t = if *left { text.trim_start() } else { text };
                let t = if *right { t.trim_end() } else { t };
                t.to_owned()
            }
            Normalizer::Nfc => text.nfc().collect(),
            Normalizer::Nfd => text.nfd().collect(),
            Normalizer::Nfkc => text.nfkc().collect(),
            Normalizer::Nfkd => text.nfkd().collect(),
            // Znak po znaku (jak HF), bez kontekstowej sigmy końcowej `str::to_lowercase`.
            Normalizer::Lowercase => text.chars().flat_map(char::to_lowercase).collect(),
        }
    }
}

/// Kiedy `Metaspace` dokleja znak zastępczy na początku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prepend {
    /// Zawsze (każdy fragment niezaczynający się od znaku zastępczego).
    Always,
    /// Tylko fragment z początku tekstu.
    First,
    /// Nigdy.
    Never,
}

/// Pre-tokenizer.
#[derive(Debug, Clone)]
pub enum PreTokenizer {
    /// Spacje → `replacement`, doklejenie na początku, podział przed znakiem zastępczym.
    Metaspace {
        /// Znak zastępczy (`▁`).
        replacement: char,
        /// Doklejanie.
        prepend: Prepend,
        /// Czy dzielić.
        split: bool,
    },
    /// Podział na białych znakach (usuwane).
    WhitespaceSplit,
}

/// Fragment tekstu po pre-tokenizacji: treść + czy zaczyna się na początku tekstu źródłowego.
pub type Piece = (String, bool);

impl PreTokenizer {
    /// Parsuje element `pre_tokenizer` (`null` → pusta lista).
    pub fn parse(v: &Value) -> Result<Vec<PreTokenizer>, EmbedError> {
        if v.is_null() {
            return Ok(Vec::new());
        }
        let one = match kind(v)? {
            "Sequence" => {
                let items = v
                    .get("pretokenizers")
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad("Sequence bez `pretokenizers`"))?;
                let mut out = Vec::new();
                for item in items {
                    out.extend(Self::parse(item)?);
                }
                return Ok(out);
            }
            "WhitespaceSplit" => PreTokenizer::WhitespaceSplit,
            "Metaspace" => {
                let replacement = str_field(v, "replacement")
                    .and_then(|s| {
                        let mut it = s.chars();
                        it.next().filter(|_| it.next().is_none())
                    })
                    .ok_or_else(|| bad("Metaspace: `replacement` musi być jednym znakiem"))?;
                let legacy = v.get("add_prefix_space").and_then(Value::as_bool);
                let prepend = match (str_field(v, "prepend_scheme"), legacy) {
                    (Some("always"), _) | (None, Some(true) | None) => Prepend::Always,
                    (Some("first"), _) => Prepend::First,
                    (Some("never"), _) | (None, Some(false)) => Prepend::Never,
                    (Some(other), _) => return Err(bad(format!("Metaspace: schemat `{other}`"))),
                };
                let split = v.get("split").and_then(Value::as_bool).unwrap_or(true);
                PreTokenizer::Metaspace {
                    replacement,
                    prepend,
                    split,
                }
            }
            other => return Err(bad(format!("nieobsługiwany pre-tokenizer `{other}`"))),
        };
        Ok(vec![one])
    }

    /// Dzieli fragmenty (puste fragmenty są usuwane, jak w HF).
    pub fn apply(&self, pieces: Vec<Piece>) -> Vec<Piece> {
        let mut out = Vec::with_capacity(pieces.len());
        for (text, at_start) in pieces {
            match self {
                PreTokenizer::WhitespaceSplit => {
                    let mut first = at_start && !text.starts_with(char::is_whitespace);
                    for word in text.split(char::is_whitespace).filter(|w| !w.is_empty()) {
                        out.push((word.to_owned(), first));
                        first = false;
                    }
                }
                PreTokenizer::Metaspace {
                    replacement,
                    prepend,
                    split,
                } => {
                    let mut s = text.replace(' ', &replacement.to_string());
                    let add = match prepend {
                        Prepend::Always => true,
                        Prepend::First => at_start,
                        Prepend::Never => false,
                    };
                    if add && !s.starts_with(*replacement) {
                        s.insert(0, *replacement);
                    }
                    if *split {
                        let mut first = at_start;
                        for part in split_merged_with_next(&s, *replacement) {
                            out.push((part, first));
                            first = false;
                        }
                    } else if !s.is_empty() {
                        out.push((s, at_start));
                    }
                }
            }
        }
        out
    }
}

/// Podział „delimiter scalony z następnym” (HF `MergedWithNext`): nowy fragment zaczyna się na
/// znaku zastępczym, chyba że poprzedni znak też nim był.
fn split_merged_with_next(text: &str, delim: char) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut previous_delim = false;
    for c in text.chars() {
        let is_delim = c == delim;
        match out.last_mut() {
            Some(last) if !is_delim || previous_delim => last.push(c),
            _ => out.push(c.to_string()),
        }
        previous_delim = is_delim;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merged_with_next_keeps_runs_of_delimiters() {
        assert_eq!(split_merged_with_next("▁a▁b", '▁'), ["▁a", "▁b"]);
        assert_eq!(split_merged_with_next("a▁▁b", '▁'), ["a", "▁▁b"]);
        assert_eq!(split_merged_with_next("▁", '▁'), ["▁"]);
        assert!(split_merged_with_next("", '▁').is_empty());
    }

    #[test]
    fn unknown_elements_fail_closed() {
        assert!(Normalizer::parse(&json!({"type": "BertNormalizer"})).is_err());
        assert!(PreTokenizer::parse(&json!({"type": "ByteLevel"})).is_err());
        assert!(PreTokenizer::parse(&json!({"type": "Metaspace", "replacement": "ab"})).is_err());
        assert!(
            Normalizer::parse(&json!({"type": "Replace", "pattern": {}, "content": ""})).is_err()
        );
        assert!(Normalizer::parse(&json!({"no_type": 1})).is_err());
    }

    #[test]
    fn simple_normalizers() {
        let parse = |v| Normalizer::parse(&v).unwrap().remove(0);
        let strip = parse(json!({"type": "Strip", "strip_left": true, "strip_right": false}));
        assert_eq!(strip.apply("  a  "), "a  ");
        let lit = parse(json!({"type": "Replace", "pattern": {"String": "."}, "content": "!"}));
        assert_eq!(lit.apply("a.b"), "a!b");
        assert_eq!(parse(json!({"type": "Lowercase"})).apply("ŻÓŁW"), "żółw");
        assert_eq!(parse(json!({"type": "NFKC"})).apply("ﬁ"), "fi");
        assert_eq!(parse(json!({"type": "NFC"})).apply("a\u{328}"), "ą");
        assert_eq!(parse(json!({"type": "NFD"})).apply("ą"), "a\u{328}");
        assert_eq!(parse(json!({"type": "NFKD"})).apply("ﬁ"), "fi");
        let empty = json!({"type": "Precompiled", "precompiled_charsmap": ""});
        assert!(Normalizer::parse(&empty).unwrap().is_empty());
        assert!(Normalizer::parse(&Value::Null).unwrap().is_empty());
    }

    #[test]
    fn metaspace_prepend_schemes() {
        let meta = |scheme: &str| {
            PreTokenizer::parse(&json!({"type": "Metaspace", "replacement": "▁", "prepend_scheme": scheme, "split": true}))
                .unwrap()
                .remove(0)
        };
        let input = vec![("a b".to_owned(), true), ("c".to_owned(), false)];
        let texts = |p: Vec<Piece>| p.into_iter().map(|(t, _)| t).collect::<Vec<_>>();
        assert_eq!(
            texts(meta("always").apply(input.clone())),
            ["▁a", "▁b", "▁c"]
        );
        assert_eq!(texts(meta("first").apply(input.clone())), ["▁a", "▁b", "c"]);
        assert_eq!(texts(meta("never").apply(input.clone())), ["a", "▁b", "c"]);
        let no_split = PreTokenizer::parse(&json!({"type": "Metaspace", "replacement": "▁", "add_prefix_space": false, "split": false}))
            .unwrap()
            .remove(0);
        assert_eq!(texts(no_split.apply(input)), ["a▁b", "c"]);
        assert!(
            PreTokenizer::parse(
                &json!({"type": "Metaspace", "replacement": "▁", "prepend_scheme": "x"})
            )
            .is_err()
        );
    }
}
