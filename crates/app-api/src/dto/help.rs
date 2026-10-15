//! DTO wbudowanej instrukcji (Ustawienia → Instrukcja) — `types-help.ts`.

use serde::{Deserialize, Serialize};

/// Dział instrukcji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HelpTopic {
    pub id: String,
    pub title: String,
}

/// Dział z treścią: HTML zsanitowany w Rust (pulldown-cmark + ammonia, ADR 0009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HelpPage {
    pub id: String,
    pub title: String,
    pub html_sanitized: String,
}

/// Trafienie wyszukiwania: dział i fragment tekstu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HelpHit {
    pub id: String,
    pub title: String,
    /// Nagłówek sekcji z trafieniem (pusty — przed pierwszym nagłówkiem).
    pub section: String,
    pub snippet: String,
}
