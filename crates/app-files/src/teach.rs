//! Plik do „Naucz Alfę” (P5): natywny dialog wyboru, te same zabezpieczenia ścieżki co
//! załączniki ([`PathGuard`]: deny-listy, dane Alfy, katalogi poświadczeń), limit rozmiaru
//! i wyciągnięcie tekstu (txt/md wprost, pdf/docx ekstraktorami załączników). Tekst wraca do UI
//! do podglądu; zapis faktów (jako treść niezaufana) robi `app-memory`.

use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use app_api::dto::{AttachmentRejectReason, TeachFile};
use app_api::error::AppError;

use crate::attach::{Extracted, document};
use crate::{FilesApp, blocking};

/// Największy plik do nauki (bajty).
pub const TEACH_MAX_BYTES: u64 = 25 * 1024 * 1024;
/// Najwięcej znaków tekstu z pliku (reszta ucięta — limit nauki w `app-memory`).
pub const TEACH_MAX_CHARS: usize = 100_000;
/// Obsługiwane rozszerzenia.
pub const TEACH_EXTENSIONS: [&str; 5] = ["txt", "md", "markdown", "pdf", "docx"];

fn extension(name: &str) -> Option<String> {
    let (_, ext) = name.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}

fn reject(reason: AttachmentRejectReason) -> AppError {
    AppError::invalid(match reason {
        AttachmentRejectReason::Denied => {
            "Tego pliku nie mogę czytać (katalog chroniony albo dane Alfy)."
        }
        AttachmentRejectReason::TooLarge => "Plik jest za duży do nauki (najwyżej 25 MB).",
        AttachmentRejectReason::Empty => "Plik jest pusty.",
        AttachmentRejectReason::NotAFile => "To nie jest zwykły plik.",
        _ => "Nie udało się odczytać pliku.",
    })
}

/// Pierwsze `max` znaków i informacja, czy ucięto.
fn cap(text: String, max: usize) -> (String, bool) {
    match text.char_indices().nth(max) {
        Some((i, _)) => (text[..i].to_owned(), true),
        None => (text, false),
    }
}

/// Tekst pliku po nazwie i treści (txt/md jako UTF-8, pdf/docx — ekstraktory załączników).
pub fn teach_text(name: &str, bytes: &[u8], max_chars: usize) -> Result<TeachFile, AppError> {
    let ext = extension(name).unwrap_or_default();
    if !TEACH_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::invalid(
            "Z pliku uczę się tylko z formatów .txt, .md, .pdf i .docx.",
        ));
    }
    let (text, truncated) = if matches!(ext.as_str(), "pdf" | "docx") {
        match document(name, bytes, max_chars) {
            Some(Extracted::Text { text, truncated }) => {
                let (text, cut) = cap(text, max_chars);
                (text, truncated || cut)
            }
            Some(Extracted::Note(note)) => return Err(AppError::invalid(note)),
            None => return Err(AppError::invalid("Nie znalazłam tekstu w tym dokumencie.")),
        }
    } else {
        let head = &bytes[..bytes.len().min(artifacts_contract::BINARY_SNIFF_BYTES)];
        if artifacts_contract::looks_binary(head) {
            return Err(AppError::invalid("To nie wygląda na plik tekstowy."));
        }
        cap(String::from_utf8_lossy(bytes).into_owned(), max_chars)
    };
    if text.trim().is_empty() {
        return Err(AppError::invalid("Nie znalazłam tekstu w tym pliku."));
    }
    Ok(TeachFile {
        name: name.to_owned(),
        chars: text.chars().count() as u64,
        text,
        truncated,
    })
}

impl FilesApp {
    /// `memory_teach_pick_file`: natywny dialog, sprawdzenie ścieżki i tekst pliku (`None` —
    /// anulowano).
    pub async fn teach_pick_file(self: &Arc<Self>) -> Result<Option<TeachFile>, AppError> {
        let me = self.clone();
        blocking(move || {
            let Some(path) = me.shell.pick_files()?.into_iter().next() else {
                return Ok(None);
            };
            me.teach_read(&path).map(Some)
        })
        .await
    }

    /// Tekst pliku do nauki (ścieżka jak dla załącznika; bez kopiowania do sesji).
    pub fn teach_read(&self, path: &Path) -> Result<TeachFile, AppError> {
        let size = self.attachments.guard().check(path).map_err(reject)?;
        if size > TEACH_MAX_BYTES {
            return Err(reject(AttachmentRejectReason::TooLarge));
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|f| f.take(TEACH_MAX_BYTES + 1).read_to_end(&mut bytes))
            .map_err(|_| reject(AttachmentRejectReason::Unreadable))?;
        if bytes.len() as u64 > TEACH_MAX_BYTES {
            return Err(reject(AttachmentRejectReason::TooLarge));
        }
        if bytes.is_empty() {
            return Err(reject(AttachmentRejectReason::Empty));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "plik".to_owned());
        teach_text(&name, &bytes, TEACH_MAX_CHARS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_files_are_read_and_capped() {
        let f = teach_text("Notatki.MD", "# Tytuł\n\nżółć".as_bytes(), 100).unwrap();
        assert_eq!(f.text, "# Tytuł\n\nżółć");
        assert_eq!(f.chars, 13);
        assert!(!f.truncated);
        let f = teach_text("a.txt", "ąąąąą".as_bytes(), 3).unwrap();
        assert_eq!((f.text.as_str(), f.truncated, f.chars), ("ąąą", true, 3));
    }

    #[test]
    fn unsupported_binary_and_empty_are_rejected() {
        assert!(teach_text("obraz.png", b"\x89PNG", 10).is_err());
        assert!(teach_text("bez-rozszerzenia", b"tekst", 10).is_err());
        assert!(teach_text("a.txt", b"\0\0\0\x01binarne", 10).is_err());
        assert!(teach_text("a.txt", b"  \n ", 10).is_err());
        assert!(teach_text("a.pdf", b"to nie jest pdf", 10).is_err());
        assert!(teach_text("a.docx", b"to nie jest docx", 10).is_err());
    }
}
