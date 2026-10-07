//! Strażnik sekretów zwykłej paczki (docs/formats/alfa-package.md §4, test „szpiegowski”).
//!
//! Dwie warstwy: (1) **redakcja** wartości — dokładne wartości z `SecretStore` i ciągi pasujące do
//! wzorców `core_log_contract::RegexRedactor`, na poziomie wartości (JSON/NDJSON/TOML parsowane,
//! żeby nie zepsuć składni) albo tekstu; wartości pod kluczami typu `api_key`/`token`/`password`
//! są redagowane zawsze; (2) **ostatnia linia obrony** — surowe bajty każdego wpisu są skanowane
//! przed zapisem i przy trafieniu eksport jest przerywany ([`crate::TransferError::SecretDetected`]).

use std::borrow::Cow;
use std::sync::LazyLock;

use accounts_hub_contract::SecretString;
use core_log_contract::{REDACTED, Redactor, RegexRedactor};
use regex::Regex;

use crate::error::TransferError;

/// Minimalna długość wartości sekretu dopasowywanej dokładnie (krótsze dawałyby fałszywe trafienia).
pub const MIN_EXACT_SECRET_CHARS: usize = 8;

/// Klucz, pod którym wartość jest zawsze redagowana (`api_key`, `access_token`, `password`…).
static SENSITIVE_KEY: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"(?i)^(.*[_.-])?(api[_-]?key|apikey|token|secret|password|passwd|credentials?)$")
        .ok()
});

/// Strażnik: znane wartości sekretów + wzorce.
pub struct SecretGuard {
    values: Vec<SecretString>,
    redactor: RegexRedactor,
}

impl std::fmt::Debug for SecretGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretGuard({} wartości)", self.values.len())
    }
}

impl SecretGuard {
    /// Strażnik ze znanymi wartościami (krótsze niż [`MIN_EXACT_SECRET_CHARS`] są pomijane).
    pub fn new(values: Vec<SecretString>) -> Self {
        let values = values
            .into_iter()
            .filter(|v| v.expose_secret().chars().count() >= MIN_EXACT_SECRET_CHARS)
            .collect();
        Self {
            values,
            redactor: RegexRedactor::default(),
        }
    }

    /// Czy klucz jest „wrażliwy” (wartość redagowana zawsze).
    pub fn is_sensitive_key(key: &str) -> bool {
        SENSITIVE_KEY.as_ref().is_some_and(|re| re.is_match(key))
    }

    /// Redaguje tekst; zwraca liczbę zredagowanych ciągów.
    pub fn clean_text<'a>(&self, text: &'a str) -> (Cow<'a, str>, u64) {
        let mut out = Cow::Borrowed(text);
        let mut count = 0;
        for v in &self.values {
            let n = out.matches(v.expose_secret()).count() as u64;
            if n > 0 {
                count += n;
                out = Cow::Owned(out.replace(v.expose_secret(), REDACTED));
            }
        }
        let before = out.matches(REDACTED).count() as u64;
        if let Cow::Owned(clean) = self.redactor.redact(&out) {
            count += (clean.matches(REDACTED).count() as u64)
                .saturating_sub(before)
                .max(1);
            out = Cow::Owned(clean);
        }
        (out, count)
    }

    /// Redaguje wartości JSON (rekurencyjnie; wrażliwe klucze zawsze).
    pub fn clean_json(&self, value: &mut serde_json::Value) -> u64 {
        match value {
            serde_json::Value::String(s) => self.clean_string(s),
            serde_json::Value::Array(items) => items.iter_mut().map(|v| self.clean_json(v)).sum(),
            serde_json::Value::Object(map) => map
                .iter_mut()
                .map(|(k, v)| {
                    if Self::is_sensitive_key(k) && !is_blank_json(v) {
                        redact_json(v)
                    } else {
                        self.clean_json(v)
                    }
                })
                .sum(),
            _ => 0,
        }
    }

    /// Redaguje wartości TOML (rekurencyjnie; wrażliwe klucze zawsze).
    pub fn clean_toml(&self, value: &mut toml::Value) -> u64 {
        match value {
            toml::Value::String(s) => self.clean_string(s),
            toml::Value::Array(items) => items.iter_mut().map(|v| self.clean_toml(v)).sum(),
            toml::Value::Table(table) => table
                .iter_mut()
                .map(|(k, v)| {
                    let blank =
                        matches!(v, toml::Value::String(s) if s.is_empty() || s == REDACTED);
                    if Self::is_sensitive_key(k) && !v.is_table() && !blank {
                        *v = toml::Value::String(REDACTED.to_owned());
                        1
                    } else {
                        self.clean_toml(v)
                    }
                })
                .sum(),
            _ => 0,
        }
    }

    fn clean_string(&self, s: &mut String) -> u64 {
        let (clean, count) = self.clean_text(s);
        if let Cow::Owned(clean) = clean {
            *s = clean;
        }
        count
    }

    /// Redaguje dokument wg rozszerzenia (`.toml`, `.json`, `.ndjson`, inne tekstowe, binarne).
    /// Zwraca nową treść i liczbę redakcji; binarne z dokładną wartością sekretu → błąd.
    pub fn clean_document(
        &self,
        path: &str,
        bytes: Vec<u8>,
    ) -> Result<(Vec<u8>, u64), TransferError> {
        let ext = path
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let Ok(text) = std::str::from_utf8(&bytes) else {
            if self
                .values
                .iter()
                .any(|v| contains(&bytes, v.expose_secret().as_bytes()))
            {
                return Err(TransferError::SecretDetected {
                    path: path.to_owned(),
                });
            }
            return Ok((bytes, 0));
        };
        let cleaned = match ext.as_str() {
            "toml" => toml::from_str::<toml::Table>(text).ok().map(|table| {
                let mut value = toml::Value::Table(table);
                let n = self.clean_toml(&mut value);
                (n, toml::to_string(&value).ok())
            }),
            "json" => serde_json::from_str::<serde_json::Value>(text)
                .ok()
                .map(|mut v| {
                    let n = self.clean_json(&mut v);
                    (n, serde_json::to_string_pretty(&v).ok())
                }),
            "ndjson" => self.clean_ndjson(text),
            _ => None,
        };
        match cleaned {
            Some((0, _)) => Ok((bytes, 0)),
            Some((n, Some(out))) => Ok((out.into_bytes(), n)),
            _ => {
                let (out, n) = self.clean_text(text);
                Ok((out.into_owned().into_bytes(), n))
            }
        }
    }

    fn clean_ndjson(&self, text: &str) -> Option<(u64, Option<String>)> {
        let mut total = 0;
        let mut out = String::with_capacity(text.len());
        for line in text.lines() {
            if line.trim().is_empty() {
                out.push_str(line);
            } else {
                let mut v: serde_json::Value = serde_json::from_str(line).ok()?;
                let n = self.clean_json(&mut v);
                total += n;
                if n == 0 {
                    out.push_str(line);
                } else {
                    out.push_str(&serde_json::to_string(&v).ok()?);
                }
            }
            out.push('\n');
        }
        Some((total, Some(out)))
    }

    /// Ostatnia linia obrony: czy surowe bajty wpisu zawierają sekret (dokładna wartość —
    /// także w postaci ucieczki JSON — albo, dla treści tekstowej UTF-8, ciąg pasujący do wzorców).
    pub fn find_leak(&self, bytes: &[u8]) -> bool {
        for v in &self.values {
            let raw = v.expose_secret();
            if contains(bytes, raw.as_bytes()) {
                return true;
            }
            let escaped = serde_json::to_string(raw).unwrap_or_default();
            let escaped = escaped.trim_matches('"');
            if escaped != raw && contains(bytes, escaped.as_bytes()) {
                return true;
            }
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return false;
        };
        // Znacznik redakcji nie jest wyciekiem (np. `api_key = "[REDACTED]"` pasuje do wzorca pary).
        let neutral = text.replace(REDACTED, "~");
        matches!(self.redactor.redact(&neutral), Cow::Owned(_))
    }
}

fn is_blank_json(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.is_empty() || s == REDACTED,
        _ => false,
    }
}

fn redact_json(v: &mut serde_json::Value) -> u64 {
    match v {
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            // Struktura pod wrażliwym kluczem: redagujemy wszystkie liście.
            let mut n = 0;
            if let serde_json::Value::Object(map) = v {
                for inner in map.values_mut() {
                    n += redact_json(inner);
                }
            } else if let serde_json::Value::Array(items) = v {
                for inner in items {
                    n += redact_json(inner);
                }
            }
            n
        }
        serde_json::Value::Null => 0,
        other => {
            *other = serde_json::Value::String(REDACTED.to_owned());
            1
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && memchr::memmem::find(haystack, needle).is_some()
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
