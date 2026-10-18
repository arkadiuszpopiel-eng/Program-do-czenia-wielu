//! Tekst dokumentów dla modelu (C2.11): DOCX / XLSX / PPTX (`office`) i PDF (`pdf`). Dokument
//! daje tekst albo notkę po polsku (np. PDF z hasłem, skan bez warstwy tekstowej).

/// Wynik ekstrakcji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extracted {
    /// Tekst dokumentu; `truncated` — ucięty limitem (znaków, stron, czasu…).
    Text {
        /// Tekst.
        text: String,
        /// Czy ucięty.
        truncated: bool,
    },
    /// Tekstu nie ma albo nie da się go wyciągnąć — wyjaśnienie dla modelu i użytkownika.
    Note(String),
}

/// Tekst dokumentu po nazwie i treści (`None` — nie dokument Office ani PDF, albo pusty Office).
pub fn document(name: &str, bytes: &[u8], max_chars: usize) -> Option<Extracted> {
    if let Some(text) = super::office::text(name, bytes) {
        return Some(Extracted::Text {
            text,
            truncated: false,
        });
    }
    super::pdf::text(name, bytes, max_chars)
}
