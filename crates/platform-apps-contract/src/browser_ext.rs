//! Rozszerzenia przeglądarki Alfy (C6.4) — typy operacji potrzebnych do pracy ze stronami
//! i testowania pisanych stron: klawisze, nawigacja w historii, wpisy konsoli i błędy skryptów.
//! Metody są w [`crate::BrowserPort`]; tu tylko typy i limity.

use serde::{Deserialize, Serialize};

/// Najwięcej zapamiętanych wpisów konsoli na sesję (starsze wypadają).
pub const MAX_CONSOLE_ENTRIES: usize = 500;
/// Najdłuższy tekst jednego wpisu konsoli.
pub const MAX_CONSOLE_TEXT: usize = 2_000;
/// Najdłuższe czekanie na tekst (ms).
pub const MAX_WAIT_MS: u64 = 30_000;
/// Największe przewinięcie jednym wywołaniem (px).
pub const MAX_SCROLL_PX: i32 = 20_000;

/// Klawisz specjalny (bez skrótów systemowych i bez wpisywania tekstu — to `type_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrowserKey {
    /// Enter.
    Enter,
    /// Tab (następne pole).
    Tab,
    /// Shift+Tab (poprzednie pole).
    ShiftTab,
    /// Escape (zamknięcie okna strony, menu).
    Escape,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Strzałka w górę.
    ArrowUp,
    /// Strzałka w dół.
    ArrowDown,
    /// Strzałka w lewo.
    ArrowLeft,
    /// Strzałka w prawo.
    ArrowRight,
    /// Strona w górę.
    PageUp,
    /// Strona w dół.
    PageDown,
    /// Początek.
    Home,
    /// Koniec.
    End,
    /// Spacja.
    Space,
}

impl BrowserKey {
    /// `key`, `code`, kod wirtualny Windows i tekst (dla klawiszy wpisujących znak).
    pub fn cdp(self) -> (&'static str, &'static str, u32, Option<&'static str>) {
        match self {
            Self::Enter => ("Enter", "Enter", 13, Some("\r")),
            Self::Tab | Self::ShiftTab => ("Tab", "Tab", 9, None),
            Self::Escape => ("Escape", "Escape", 27, None),
            Self::Backspace => ("Backspace", "Backspace", 8, None),
            Self::Delete => ("Delete", "Delete", 46, None),
            Self::ArrowUp => ("ArrowUp", "ArrowUp", 38, None),
            Self::ArrowDown => ("ArrowDown", "ArrowDown", 40, None),
            Self::ArrowLeft => ("ArrowLeft", "ArrowLeft", 37, None),
            Self::ArrowRight => ("ArrowRight", "ArrowRight", 39, None),
            Self::PageUp => ("PageUp", "PageUp", 33, None),
            Self::PageDown => ("PageDown", "PageDown", 34, None),
            Self::Home => ("Home", "Home", 36, None),
            Self::End => ("End", "End", 35, None),
            Self::Space => (" ", "Space", 32, Some(" ")),
        }
    }

    /// Modyfikatory CDP (Shift = 8).
    pub fn modifiers(self) -> u32 {
        if self == Self::ShiftTab { 8 } else { 0 }
    }
}

/// Nawigacja w historii karty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HistoryNav {
    /// Wstecz.
    Back,
    /// Dalej.
    Forward,
    /// Odśwież.
    Reload,
}

/// Poziom wpisu konsoli.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleLevel {
    /// `console.debug`.
    Debug,
    /// `console.log`.
    Log,
    /// `console.info`.
    Info,
    /// `console.warn`.
    Warning,
    /// `console.error`, nieobsłużony wyjątek, błąd zasobu.
    Error,
}

/// Wpis konsoli strony (treść strony — niezaufana).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConsoleEntry {
    /// Poziom.
    pub level: ConsoleLevel,
    /// Źródło: `console`, `exception` (nieobsłużony wyjątek), `network`, `log`.
    pub source: String,
    /// Treść (≤ [`MAX_CONSOLE_TEXT`] znaków).
    pub text: String,
    /// Adres skryptu albo zasobu.
    pub url: Option<String>,
    /// Wiersz w skrypcie (od 1).
    pub line: Option<u32>,
}

impl ConsoleEntry {
    /// Wpis z tekstem obciętym do limitu.
    pub fn new(level: ConsoleLevel, source: &str, text: &str) -> Self {
        Self {
            level,
            source: source.to_owned(),
            text: text.chars().take(MAX_CONSOLE_TEXT).collect(),
            url: None,
            line: None,
        }
    }
}

/// Poziom z nazwy CDP (`log`, `warning`, `error`, `assert`…).
pub fn console_level(name: &str) -> ConsoleLevel {
    match name {
        "debug" | "verbose" => ConsoleLevel::Debug,
        "info" => ConsoleLevel::Info,
        "warning" | "warn" => ConsoleLevel::Warning,
        "error" | "assert" => ConsoleLevel::Error,
        _ => ConsoleLevel::Log,
    }
}
