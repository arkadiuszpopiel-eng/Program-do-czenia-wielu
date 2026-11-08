//! Redakcja sekretów przed zapisem do logów.

use std::borrow::Cow;

use regex::Regex;

/// Tekst zastępujący sekret.
pub const REDACTED: &str = "[REDACTED]";

/// Redaktor sekretów: działa na tekście i rekurencyjnie na wartościach JSON.
pub trait Redactor: Send + Sync {
    /// Redaguje sekrety w tekście; zwraca `Borrowed`, gdy nic nie zmieniono.
    fn redact<'a>(&self, input: &'a str) -> Cow<'a, str>;

    /// Redaguje wszystkie łańcuchy w wartości JSON (klucze obiektów bez zmian).
    fn redact_value(&self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(s) => {
                if let Cow::Owned(clean) = self.redact(s) {
                    *s = clean;
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|v| self.redact_value(v)),
            serde_json::Value::Object(map) => map.values_mut().for_each(|v| self.redact_value(v)),
            _ => {}
        }
    }
}

/// Redaktor oparty na wyrażeniach regularnych dla znanych formatów kluczy i tokenów.
#[derive(Debug, Clone)]
pub struct RegexRedactor {
    patterns: Vec<Regex>,
}

/// Wzorce domyślne (kolejność: dłuższe/bardziej specyficzne najpierw).
const DEFAULT_PATTERNS: [&str; 7] = [
    r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}", // nagłówek Authorization
    r"\bsk-[A-Za-z0-9_-]{8,}",                // OpenAI / Anthropic (`sk-ant-…`)
    r"\bxai-[A-Za-z0-9_-]{8,}",               // xAI
    r"\bAIza[0-9A-Za-z_-]{20,}",              // Google API key
    r"\bgh[pousr]_[A-Za-z0-9]{20,}",          // GitHub tokens
    r"\bxox[abprs]-[A-Za-z0-9-]{10,}",        // Slack
    r"(?i)\b(api[_-]?key|token|secret|password)\s*[=:]\s*\S{6,}", // pary klucz=wartość
];

impl RegexRedactor {
    /// Redaktor z własną listą wzorców (błędny wzorzec → `Err`).
    pub fn new<'a>(patterns: impl IntoIterator<Item = &'a str>) -> Result<Self, regex::Error> {
        let patterns = patterns
            .into_iter()
            .map(Regex::new)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { patterns })
    }

    /// Liczba aktywnych wzorców.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

impl Default for RegexRedactor {
    /// Wzorce domyślne; są stałymi sprawdzonymi testem, więc kompilacja nie zawodzi.
    fn default() -> Self {
        let patterns = DEFAULT_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();
        Self { patterns }
    }
}

impl Redactor for RegexRedactor {
    fn redact<'a>(&self, input: &'a str) -> Cow<'a, str> {
        let mut current = Cow::Borrowed(input);
        for re in &self.patterns {
            if re.is_match(&current) {
                current = Cow::Owned(re.replace_all(&current, REDACTED).into_owned());
            }
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_default_patterns_compile() {
        assert_eq!(
            RegexRedactor::default().pattern_count(),
            DEFAULT_PATTERNS.len()
        );
    }

    #[test]
    fn redacts_known_key_formats() {
        let r = RegexRedactor::default();
        let cases = [
            "klucz sk-ant-api03-abcdefghijklmnop koniec",
            "xai-ABCDEFGH12345678",
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.abc.def",
            "AIzaSyA1234567890abcdefghijklmnop",
            "ghp_abcdefghijklmnopqrstuvwxyz1234",
            "xoxb-1234567890-abcdefgh",
            "api_key = supersecret123",
        ];
        for case in cases {
            let out = r.redact(case);
            assert!(out.contains(REDACTED), "nie zredagowano: {case}");
            assert!(
                !out.contains("secret123") && !out.contains("abcdefgh"),
                "{out}"
            );
        }
    }

    #[test]
    fn leaves_ordinary_text_untouched() {
        let r = RegexRedactor::default();
        let text = "sklep skarpetki sk-1 bearer of news token: ok";
        assert!(matches!(r.redact(text), Cow::Borrowed(_)));
    }

    #[test]
    fn redacts_nested_json() {
        let r = RegexRedactor::default();
        let mut v = serde_json::json!({
            "headers": {"authorization": "Bearer abcdefghijklmnop"},
            "list": ["ok", "sk-1234567890abcdef"],
            "n": 5
        });
        r.redact_value(&mut v);
        assert_eq!(v["headers"]["authorization"], serde_json::json!(REDACTED));
        assert_eq!(v["list"][1], serde_json::json!(REDACTED));
        assert_eq!(v["list"][0], serde_json::json!("ok"));
        assert_eq!(v["n"], serde_json::json!(5));
    }

    #[test]
    fn invalid_custom_pattern_is_error() {
        assert!(RegexRedactor::new(["("]).is_err());
    }
}
