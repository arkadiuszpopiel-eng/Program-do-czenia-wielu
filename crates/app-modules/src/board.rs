//! Tablica (P4): do 40 przypiętych odpowiedzi z różnych rozmów — kopia treści (Markdown, do
//! 4000 znaków) z nazwą rozmowy, agentką i datą, w pliku `board.json` katalogu stanu. Kopia, bo
//! historia rozmowy jest tylko do dopisywania, a rozmowę można usunąć; przypięcie nie zmienia
//! rozmowy. HTML dla widoku renderuje Rust (`lib-markdown`), jak odpowiedzi.

use std::path::Path;

use app_api::AppError;
use app_api::dto::BoardItem;
use serde::{Deserialize, Serialize};

/// Nazwa pliku w katalogu stanu.
pub const FILE: &str = "board.json";
/// Najwięcej przypięć.
pub const MAX_ITEMS: usize = 40;
/// Najdłuższa treść przypięcia (znaki).
pub const MAX_TEXT: usize = 4000;

/// Wpis w pliku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    /// Identyfikator przypięcia.
    pub id: String,
    /// Rozmowa.
    pub session_id: String,
    /// Nazwa rozmowy w chwili przypięcia.
    pub session_title: String,
    /// Tura.
    pub turn_id: String,
    /// Agentka.
    pub agent: String,
    /// Treść (Markdown, do 4000 znaków).
    pub text: String,
    /// Czas przypięcia (RFC 3339).
    pub pinned_at: String,
}

/// Odczyt; brak pliku albo zły format — pusta tablica.
pub fn load(path: &Path) -> Vec<Pin> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save(path: &Path, pins: &[Pin]) -> Result<(), AppError> {
    let text = serde_json::to_string_pretty(pins).map_err(|e| AppError::internal(e.to_string()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(AppError::storage)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(AppError::storage)?;
    std::fs::rename(&tmp, path).map_err(AppError::storage)
}

/// Widok: najnowsze przypięcia pierwsze, treść jako HTML zsanitowany.
pub fn view(pins: &[Pin]) -> Vec<BoardItem> {
    let mut out: Vec<BoardItem> = pins
        .iter()
        .map(|p| BoardItem {
            id: p.id.clone(),
            session_id: p.session_id.clone(),
            session_title: p.session_title.clone(),
            turn_id: p.turn_id.clone(),
            agent: p.agent.clone(),
            text: p.text.clone(),
            html_sanitized: lib_markdown::render(&p.text),
            pinned_at: p.pinned_at.clone(),
        })
        .collect();
    out.sort_by(|a, b| b.pinned_at.cmp(&a.pinned_at));
    out
}

/// Przypina (ta sama tura drugi raz — bez zmian); pełna tablica — odmowa z powodem.
pub fn pin(path: &Path, mut new: Pin) -> Result<Vec<BoardItem>, AppError> {
    let mut pins = load(path);
    if pins
        .iter()
        .any(|p| p.session_id == new.session_id && p.turn_id == new.turn_id)
    {
        return Ok(view(&pins));
    }
    if pins.len() >= MAX_ITEMS {
        return Err(AppError::invalid(format!(
            "Tablica jest pełna ({MAX_ITEMS} przypięć) — odepnij coś, żeby dodać nowe."
        )));
    }
    if new.text.trim().is_empty() {
        return Err(AppError::invalid(
            "Pusta wiadomość — nie ma czego przypiąć.",
        ));
    }
    new.text = new.text.chars().take(MAX_TEXT).collect();
    pins.push(new);
    save(path, &pins)?;
    Ok(view(&pins))
}

/// Odpina (nieznany identyfikator — bez zmian).
pub fn unpin(path: &Path, id: &str) -> Result<Vec<BoardItem>, AppError> {
    let mut pins = load(path);
    let before = pins.len();
    pins.retain(|p| p.id != id);
    if pins.len() != before {
        save(path, &pins)?;
    }
    Ok(view(&pins))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin_of(n: usize, text: &str) -> Pin {
        Pin {
            id: format!("p{n}"),
            session_id: "s1".into(),
            session_title: "Rozmowa".into(),
            turn_id: format!("s1:t{n}"),
            agent: "alfa".into(),
            text: text.into(),
            pinned_at: format!("2026-10-15T10:{:02}:00Z", n % 60),
        }
    }

    #[test]
    fn pin_dedupe_limit_and_unpin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let list = pin(&path, pin_of(1, "**Ważne**: termin w piątek")).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].html_sanitized.contains("<strong>Ważne</strong>"));
        assert_eq!(
            pin(&path, pin_of(1, "inna")).unwrap().len(),
            1,
            "ta sama tura"
        );
        assert!(pin(&path, pin_of(2, "  ")).is_err(), "pusta");
        for n in 2..=MAX_ITEMS {
            pin(&path, pin_of(n, "x")).unwrap();
        }
        assert!(pin(&path, pin_of(99, "x")).is_err(), "pełna tablica");
        let after = unpin(&path, "p1").unwrap();
        assert_eq!(after.len(), MAX_ITEMS - 1);
        assert!(
            after.windows(2).all(|w| w[0].pinned_at >= w[1].pinned_at),
            "najnowsze pierwsze"
        );
        let long = pin(&path, pin_of(100, &"y".repeat(MAX_TEXT + 50))).unwrap();
        assert!(long.iter().any(|i| i.text.chars().count() == MAX_TEXT));
        assert_eq!(unpin(&path, "nie-ma").unwrap().len(), MAX_ITEMS);
    }
}
