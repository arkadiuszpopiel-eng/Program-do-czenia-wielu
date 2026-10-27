//! Szczegóły błędu komendy do odczytu maszynowego (`AppError.details`, plan v6 zadanie B.3):
//! płaski słownik o stałych kluczach — rodzaj i identyfikator obiektu, nazwa pola, powód jako
//! wartość wyliczenia, liczba sekund. Nigdy treść użytkownika ani sekrety.
//!
//! Jedyna droga do słownika to [`ErrorDetails::insert`] (także przy deserializacji), a ta odrzuca
//! klucze o nazwach kojarzących się z sekretem i wartości wyglądające na sekret albo treść. To
//! druga linia obrony: pierwszą jest zasada, że miejsca zgłaszające błąd podają wyłącznie
//! identyfikatory, wartości wyliczeń i liczby.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Najwięcej wpisów w jednym błędzie.
pub const MAX_ENTRIES: usize = 8;
/// Najdłuższa nazwa klucza.
pub const MAX_KEY_LEN: usize = 32;
/// Najdłuższa wartość tekstowa (identyfikator tury z sesją ma do ~75 znaków).
pub const MAX_TEXT_LEN: usize = 96;
/// Najdłuższy dozwolony ciąg samych liter i cyfr (UUID bez myślników ma 32 znaki; klucze API
/// i żetony mają dłuższe ciągi bez separatorów).
pub const MAX_OPAQUE_RUN: usize = 32;

/// Fragmenty nazw kluczy, których szczegóły nie mogą nieść.
const FORBIDDEN_KEY_PARTS: [&str; 9] = [
    "key",
    "secret",
    "token",
    "password",
    "passwd",
    "authorization",
    "bearer",
    "cookie",
    "credential",
];

/// Początki wartości typowe dla kluczy API, żetonów i nagłówków uwierzytelnienia.
const SECRET_PREFIXES: [&str; 11] = [
    "sk-", "sk_", "pk_", "rk_", "ghp_", "gho_", "xox", "aiza", "eyj", "bearer ", "basic ",
];

/// Wartość szczegółu: tekst (identyfikator, wartość wyliczenia), liczba albo flaga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum DetailValue {
    /// Flaga.
    Flag(bool),
    /// Liczba nieujemna (sekundy, limity, liczniki).
    Number(u64),
    /// Krótki tekst: identyfikator albo wartość wyliczenia.
    Text(String),
}

impl From<bool> for DetailValue {
    fn from(value: bool) -> Self {
        Self::Flag(value)
    }
}

impl From<u64> for DetailValue {
    fn from(value: u64) -> Self {
        Self::Number(value)
    }
}

impl From<u32> for DetailValue {
    fn from(value: u32) -> Self {
        Self::Number(u64::from(value))
    }
}

impl From<&str> for DetailValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for DetailValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

/// Słownik szczegółów błędu (pusty = pole `details` nie jest wysyłane).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ErrorDetails(#[serde(deserialize_with = "filtered")] BTreeMap<String, DetailValue>);

/// Deserializacja przez ten sam filtr co [`ErrorDetails::insert`]; wpisy o nieobsługiwanym
/// kształcie (tablica, obiekt, `null`, liczba ujemna) są pomijane zamiast psuć cały błąd.
fn filtered<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, DetailValue>, D::Error> {
    let raw = BTreeMap::<String, serde_json::Value>::deserialize(deserializer)?;
    let mut out = ErrorDetails::default();
    for (key, value) in raw {
        if let Ok(value) = serde_json::from_value::<DetailValue>(value) {
            out.insert(&key, value);
        }
    }
    Ok(out.0)
}

impl ErrorDetails {
    /// Brak szczegółów.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Liczba wpisów.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Wartość pod kluczem.
    pub fn get(&self, key: &str) -> Option<&DetailValue> {
        self.0.get(key)
    }

    /// Wartość tekstowa pod kluczem.
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.0.get(key) {
            Some(DetailValue::Text(text)) => Some(text),
            _ => None,
        }
    }

    /// Wartość liczbowa pod kluczem.
    pub fn number(&self, key: &str) -> Option<u64> {
        match self.0.get(key) {
            Some(DetailValue::Number(number)) => Some(*number),
            _ => None,
        }
    }

    /// Wpisy w kolejności kluczy.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &DetailValue)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }

    /// Dodaje wpis; `false` — odrzucony (nazwa klucza albo wartość nie przeszła filtra, albo
    /// słownik jest pełny). Odrzucenie nie jest błędem: błąd komendy idzie dalej bez tego wpisu.
    pub fn insert(&mut self, key: &str, value: impl Into<DetailValue>) -> bool {
        let value = value.into();
        if !key_allowed(key) || !value_allowed(&value) {
            return false;
        }
        if self.0.len() >= MAX_ENTRIES && !self.0.contains_key(key) {
            return false;
        }
        self.0.insert(key.to_owned(), value);
        true
    }
}

/// Klucz: `snake_case` ASCII od litery, bez fragmentów z listy zabronionych.
fn key_allowed(key: &str) -> bool {
    let shape = key.len() <= MAX_KEY_LEN
        && key.starts_with(|c: char| c.is_ascii_lowercase())
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    shape && !FORBIDDEN_KEY_PARTS.iter().any(|part| key.contains(part))
}

/// Tekst: krótki, jednowierszowy, bez początku typowego dla sekretu i bez długiego ciągu samych
/// liter i cyfr (nieprzejrzysty żeton); liczby i flagi przechodzą zawsze.
fn value_allowed(value: &DetailValue) -> bool {
    let DetailValue::Text(text) = value else {
        return true;
    };
    if text.is_empty() || text.len() > MAX_TEXT_LEN || text.chars().any(char::is_control) {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if SECRET_PREFIXES.iter().any(|p| lower.starts_with(p)) || lower.contains("-----begin") {
        return false;
    }
    let mut run = 0usize;
    for c in text.chars() {
        run = if c.is_alphanumeric() { run + 1 } else { 0 };
        if run > MAX_OPAQUE_RUN {
            return false;
        }
    }
    true
}
