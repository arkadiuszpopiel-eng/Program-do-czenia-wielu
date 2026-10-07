//! Polityka treści dziennika: co z pól zdarzenia może trafić do pliku.
//!
//! Kolejność dla każdej wartości (najpierw redakcja, potem obcinanie — obcięcie nie może
//! „przepołowić” klucza tak, że wzorzec go nie rozpozna):
//! 1. pole o nazwie sekretu (`api_key`, `token`, `password`, `klucz`, `hasło`…) → `[REDACTED]`
//!    (poza wartościami logicznymi);
//! 2. pole o nazwie treści (`text`, `content`, `prompt`, `transcript`, `pixels`, `image`…) →
//!    `[pominięto: N znaków]` — treść rozmów, transkrypcje i piksele nigdy nie trafiają do pliku;
//! 3. wartość dłuższa niż [`HARD_CAP`] → pominięta w całości;
//! 4. redaktor `core-log` (`RegexRedactor::default()`) + wzorce dodatkowe + nieprzezroczyste
//!    tokeny (≥ 32 znaki z wielkimi i małymi literami oraz cyframi);
//! 5. obcięcie do limitu znaków i ucieczka znaków sterujących (jedno zdarzenie = jedna linia).

use std::borrow::Cow;

use core_log_contract::{REDACTED, Redactor, RegexRedactor};

/// Limit znaków komunikatu.
pub const MAX_MESSAGE_CHARS: usize = 4_096;
/// Limit znaków wartości pola.
pub const MAX_FIELD_CHARS: usize = 1_024;
/// Maksymalna liczba pól zdarzenia w linii.
pub const MAX_FIELDS: usize = 32;
/// Wartość dłuższa (w bajtach) nie jest nawet skanowana — pomijana w całości.
pub const HARD_CAP: usize = 64 * 1024;

/// Słowa nazw pól z sekretami (porównanie po słowach nazwy rozdzielonych `_`, `-`, `.`).
const SECRET_WORDS: [&str; 18] = [
    "key",
    "apikey",
    "token",
    "secret",
    "password",
    "passwd",
    "pwd",
    "haslo",
    "hasło",
    "klucz",
    "authorization",
    "auth",
    "cookie",
    "cookies",
    "credential",
    "credentials",
    "bearer",
    "sekret",
];

/// Słowa nazw pól z treścią użytkownika (rozmowy, mowa, ekran, schowek).
const CONTENT_WORDS: [&str; 22] = [
    "text",
    "tekst",
    "content",
    "tresc",
    "treść",
    "prompt",
    "body",
    "transcript",
    "transkrypcja",
    "utterance",
    "wypowiedz",
    "wypowiedź",
    "pixels",
    "piksele",
    "image",
    "obraz",
    "screenshot",
    "zrzut",
    "clipboard",
    "schowek",
    "samples",
    "audio",
];

/// Wzorce dodatkowe (poza domyślnymi `core-log`): tokeny GitHub PAT, AWS, JWT, klucze
/// prywatne PEM, Hugging Face, Groq, Perplexity.
const EXTRA_PATTERNS: [&str; 7] = [
    r"\bgithub_pat_[A-Za-z0-9_]{20,}",
    r"\bAKIA[0-9A-Z]{16}\b",
    r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
    r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(-----END [A-Z ]*PRIVATE KEY-----|$)",
    r"\bhf_[A-Za-z0-9]{20,}",
    r"\bgsk_[A-Za-z0-9]{20,}",
    r"\bpplx-[A-Za-z0-9]{20,}",
];

/// Minimalna długość nieprzezroczystego tokenu.
const OPAQUE_MIN: usize = 32;

fn words(name: &str) -> impl Iterator<Item = String> + '_ {
    name.split(['_', '-', '.'])
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
}

/// Czy nazwa pola wskazuje sekret.
pub fn is_secret_field(name: &str) -> bool {
    words(name).any(|w| SECRET_WORDS.contains(&w.as_str()))
}

/// Czy nazwa pola wskazuje treść użytkownika.
pub fn is_content_field(name: &str) -> bool {
    words(name).any(|w| CONTENT_WORDS.contains(&w.as_str()))
}

/// Redakcja tekstu: `core-log` + wzorce dodatkowe + nieprzezroczyste tokeny.
pub struct Redaction {
    core: RegexRedactor,
    extra: Option<RegexRedactor>,
}

impl Default for Redaction {
    fn default() -> Self {
        Self {
            core: RegexRedactor::default(),
            extra: RegexRedactor::new(EXTRA_PATTERNS).ok(),
        }
    }
}

fn opaque(token: &str) -> bool {
    token.len() >= OPAQUE_MIN
        && token.chars().any(|c| c.is_ascii_uppercase())
        && token.chars().any(|c| c.is_ascii_lowercase())
        && token.chars().any(|c| c.is_ascii_digit())
}

/// Zastępuje nieprzezroczyste tokeny (ciągi `[A-Za-z0-9_-]` ≥ 32 znaki z wielkimi i małymi
/// literami oraz cyframi — typowe klucze bez rozpoznawalnego prefiksu). Hashe hex i UUID
/// (małe litery) zostają.
fn redact_opaque(text: &str) -> Cow<'_, str> {
    let is_token = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    let mut out = String::new();
    let mut last = 0;
    let mut start: Option<usize> = None;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        match (is_token(c) && i < text.len(), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if opaque(&text[s..i]) {
                    out.push_str(&text[last..s]);
                    out.push_str(REDACTED);
                    last = i;
                }
                start = None;
            }
            _ => {}
        }
    }
    if last == 0 {
        return Cow::Borrowed(text);
    }
    out.push_str(&text[last..]);
    Cow::Owned(out)
}

impl Redaction {
    /// Redaguje sekrety w tekście.
    pub fn redact(&self, text: &str) -> String {
        let core = self.core.redact(text);
        let extra = match &self.extra {
            Some(r) => Cow::Owned(r.redact(&core).into_owned()),
            None => core,
        };
        redact_opaque(&extra).into_owned()
    }

    /// Wartość pola gotowa do zapisu (redakcja, pominięcie treści, obcięcie, ucieczka).
    pub fn field(&self, name: &str, value: &str, is_bool: bool) -> String {
        if is_secret_field(name) && !is_bool {
            return REDACTED.to_owned();
        }
        if is_content_field(name) {
            return format!("[pominięto: {} znaków]", value.chars().count());
        }
        self.clean(value, MAX_FIELD_CHARS)
    }

    /// Komunikat gotowy do zapisu.
    pub fn message(&self, message: &str) -> String {
        self.clean(message, MAX_MESSAGE_CHARS)
    }

    fn clean(&self, value: &str, max_chars: usize) -> String {
        if value.len() > HARD_CAP {
            return format!("[pominięto: {} bajtów]", value.len());
        }
        escape(&truncate(&self.redact(value), max_chars))
    }
}

/// Obcina do `max` znaków z dopiskiem liczby pominiętych.
pub fn truncate(text: &str, max: usize) -> Cow<'_, str> {
    match text.char_indices().nth(max) {
        None => Cow::Borrowed(text),
        Some((cut, _)) => {
            let rest = text[cut..].chars().count();
            Cow::Owned(format!("{}…[+{rest}]", &text[..cut]))
        }
    }
}

/// Ucieczka znaków sterujących i znaków kierunku tekstu (fałszowanie linii dziennika).
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' => {
                out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
            }
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_names() {
        for n in [
            "api_key",
            "apiKey",
            "x-api-key",
            "token",
            "access_token",
            "klucz",
            "hasło",
        ] {
            assert!(is_secret_field(n), "{n}");
        }
        for n in [
            "hotkey",
            "keyboard",
            "tokens_used",
            "model",
            "skrot",
            "error",
        ] {
            assert!(!is_secret_field(n), "{n}");
        }
        for n in [
            "text",
            "user_prompt",
            "transcript",
            "screenshot_png",
            "treść",
        ] {
            assert!(is_content_field(n), "{n}");
        }
        assert!(!is_content_field("context_len"));
    }

    #[test]
    fn values_follow_policy() {
        let r = Redaction::default();
        assert_eq!(r.field("api_key", "plain-value", false), REDACTED);
        assert_eq!(r.field("klucz", "true", true), "true");
        assert_eq!(
            r.field("prompt", "ala ma kota", false),
            "[pominięto: 11 znaków]"
        );
        let hidden = r.field("x", "klucz sk-ant-api03-abcdefghijklmnop", false);
        assert!(!hidden.contains("abcdefghijklmnop"), "{hidden}");
        let big = "a".repeat(HARD_CAP + 1);
        assert!(r.field("x", &big, false).starts_with("[pominięto:"));
        let long = "b".repeat(MAX_FIELD_CHARS + 5);
        assert!(r.field("x", &long, false).ends_with("…[+5]"));
    }

    #[test]
    fn extra_patterns_and_opaque_tokens() {
        let r = Redaction::default();
        for secret in [
            "github_pat_11ABCDEFG0123456789_abcdefghijklmnop",
            "AKIAIOSFODNN7EXAMPLE",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
            "hf_abcdefghijklmnopqrstuvwxyz",
            "gsk_abcdefghijklmnopqrstuvwxyz12",
            "Kx9vQ2mZ7pL4wR8tY1uI5oP3aS6dF0gH",
        ] {
            let out = r.redact(&format!("przed {secret} po"));
            assert!(!out.contains(secret), "{out}");
            assert!(out.starts_with("przed ") && out.ends_with(" po"), "{out}");
        }
        let pem = "-----BEGIN RSA PRIVATE KEY-----\nMIIabc\n-----END RSA PRIVATE KEY-----";
        assert!(!r.redact(pem).contains("MIIabc"));
        for keep in [
            "7b7f5436cfcb02fae583a05b512ea96467fd449fe54cb49a5e4f06c51a1e43b8",
            "0b6c1f4e-3a5d-4f7e-9b2a-1c3d5e7f9a0b",
            "C:\\Users\\Ala\\AppData\\Local\\Alfa\\sidecars\\llama-vulkan\\llama-server.exe",
            "bielik-4.5b-v3.0-instruct-q8_0",
        ] {
            assert_eq!(r.redact(keep), keep);
        }
    }

    #[test]
    fn escaping_keeps_one_event_per_line() {
        assert_eq!(escape("a\nb\r\tc"), "a\\nb\\r\\tc");
        assert_eq!(escape("x\u{1b}[31m"), "x\\u{1b}[31m");
        assert_eq!(escape("a\u{202E}b"), "a\\u{202e}b");
        assert_eq!(escape("zażółć"), "zażółć");
        assert_eq!(truncate("ąęć", 2), "ąę…[+1]");
        assert_eq!(truncate("ąę", 2), "ąę");
    }
}
