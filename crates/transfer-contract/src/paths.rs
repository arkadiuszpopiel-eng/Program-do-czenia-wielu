//! Ścieżki wpisów paczki: walidacja (ochrona przed zip-slip) i mapowanie na elementy.
//!
//! Reguła (docs/formats/alfa-package.md §1): ścieżki względne, `/` jako separator, bez `..`,
//! bez ścieżek bezwzględnych. Dodatkowo — bo paczka trafia na Windows — odrzucane są: `\`,
//! dwukropek (litera dysku `C:`, strumienie NTFS `plik:ads`), UNC (`//serwer`), nazwy
//! zarezerwowane (`CON`, `NUL`, `COM1`…), segmenty kończące się kropką lub spacją (Windows je
//! obcina — aliasy nazw) i znaki sterujące. Każda ścieżka zaakceptowana przez
//! [`validate_entry_path`] po złączeniu z katalogiem docelowym zostaje **wewnątrz** niego.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sessions_contract::SessionId;

use crate::scope::Category;

/// Ścieżka manifestu (pierwszy wpis archiwum).
pub const MANIFEST_PATH: &str = "manifest.json";
/// Wpis z danymi rollbacku w snapshocie (lista elementów utworzonych przez import).
pub const ROLLBACK_PATH: &str = "rollback.json";
/// Wpis z sekretami (tylko paczka `secrets`, zawsze szyfrowana w całości).
pub const SECRETS_PATH: &str = "secrets.json";
/// Metadane sesji w paczce.
pub const SESSION_FILE: &str = "session.json";
/// Drzewo tur sesji (NDJSON).
pub const TURNS_FILE: &str = "turns.ndjson";
/// Maksymalna długość ścieżki wpisu w bajtach.
pub const MAX_PATH_BYTES: usize = 512;
/// Maksymalna głębokość (liczba segmentów).
pub const MAX_DEPTH: usize = 16;

/// Nazwy urządzeń Windows (także `COM0`/`LPT0`, cyfry w indeksie górnym i konsola `CONIN$`/
/// `CONOUT$` — przegląd 2026-10).
const RESERVED: [&str; 32] = [
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
    "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³", "CONIN$", "CONOUT$",
];

/// Powód odrzucenia ścieżki.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PathError {
    /// Pusta ścieżka.
    #[error("pusta ścieżka")]
    Empty,
    /// Za długa albo za głęboka.
    #[error("za długa ścieżka")]
    TooLong,
    /// Ścieżka bezwzględna (`/…`).
    #[error("ścieżka bezwzględna")]
    Absolute,
    /// Ukośnik wsteczny (`\`) — separator Windows albo UNC.
    #[error("ukośnik wsteczny")]
    Backslash,
    /// Dwukropek: litera dysku (`C:`) albo strumień NTFS.
    #[error("dwukropek (litera dysku lub strumień NTFS)")]
    Colon,
    /// Segment `.` albo `..`.
    #[error("segment `.` lub `..`")]
    DotSegment,
    /// Pusty segment (`a//b`, `//serwer`, ukośnik na końcu).
    #[error("pusty segment")]
    EmptySegment,
    /// Znak sterujący.
    #[error("znak sterujący")]
    Control,
    /// Nazwa zarezerwowana w Windows (`CON`, `NUL`, `COM1`…).
    #[error("nazwa zarezerwowana w Windows")]
    Reserved,
    /// Segment kończy się kropką albo spacją (Windows je obcina).
    #[error("segment kończy się kropką lub spacją")]
    TrailingDotOrSpace,
}

/// Sprawdza ścieżkę wpisu paczki (także nazwy dokumentów z portów przed zapisem).
pub fn validate_entry_path(path: &str) -> Result<(), PathError> {
    if path.is_empty() {
        return Err(PathError::Empty);
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(PathError::TooLong);
    }
    if path.chars().any(char::is_control) {
        return Err(PathError::Control);
    }
    if path.contains('\\') {
        return Err(PathError::Backslash);
    }
    if path.starts_with('/') {
        return Err(PathError::Absolute);
    }
    if path.contains(':') {
        return Err(PathError::Colon);
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() > MAX_DEPTH {
        return Err(PathError::TooLong);
    }
    for segment in segments {
        validate_segment(segment)?;
    }
    Ok(())
}

fn validate_segment(segment: &str) -> Result<(), PathError> {
    if segment.is_empty() {
        return Err(PathError::EmptySegment);
    }
    if segment == "." || segment == ".." {
        return Err(PathError::DotSegment);
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Err(PathError::TrailingDotOrSpace);
    }
    let stem = segment.split('.').next().unwrap_or(segment).trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Err(PathError::Reserved);
    }
    Ok(())
}

/// Ścieżka dokumentu kategorii w paczce (`config/common/shared.toml`).
pub fn document_path(category: Category, name: &str) -> String {
    format!("{}/{name}", category.dir())
}

/// Ścieżka pliku sesji w paczce (`sessions/<id>/<file>`).
pub fn session_path(id: &SessionId, file: &str) -> String {
    format!("{}/{id}/{file}", Category::Sessions.dir())
}

/// Czym jest wpis paczki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    /// `manifest.json`.
    Manifest,
    /// `rollback.json` (snapshot).
    Rollback,
    /// `secrets.json` (paczka sekretów).
    Secrets,
    /// Dokument kategorii.
    Document {
        /// Kategoria.
        category: Category,
        /// Nazwa względem katalogu kategorii.
        name: String,
    },
    /// Plik sesji.
    SessionFile {
        /// Identyfikator sesji (katalog).
        id: SessionId,
        /// Nazwa pliku (`session.json`, `turns.ndjson`, `attachments/…`).
        file: String,
    },
    /// Nieznany wpis (import go pomija i wypisuje w dry-run).
    Unknown,
}

/// Klasyfikuje (już zwalidowaną) ścieżkę wpisu.
pub fn classify(path: &str) -> EntryKind {
    match path {
        MANIFEST_PATH => return EntryKind::Manifest,
        ROLLBACK_PATH => return EntryKind::Rollback,
        SECRETS_PATH => return EntryKind::Secrets,
        _ => {}
    }
    let sessions = Category::Sessions.dir();
    if let Some(rest) = path
        .strip_prefix(sessions)
        .and_then(|r| r.strip_prefix('/'))
    {
        return match rest.split_once('/') {
            Some((id, file)) if !id.is_empty() && !file.is_empty() => EntryKind::SessionFile {
                id: SessionId::new(id),
                file: file.to_owned(),
            },
            _ => EntryKind::Unknown,
        };
    }
    for category in Category::DOCUMENTS {
        if let Some(name) = path
            .strip_prefix(category.dir())
            .and_then(|r| r.strip_prefix('/'))
            .filter(|n| !n.is_empty())
        {
            return EntryKind::Document {
                category,
                name: name.to_owned(),
            };
        }
    }
    EntryKind::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_regular_paths() {
        for ok in [
            "manifest.json",
            "config/common/shared.toml",
            "personas/alfa/voice-bible.toml",
            "sessions/01933b2e-8f4a/turns.ndjson",
            "artifacts/s1/raport końcowy.pdf",
            "memory/.hidden",
        ] {
            assert_eq!(validate_entry_path(ok), Ok(()), "{ok}");
        }
    }

    #[test]
    fn rejects_traversal_and_windows_tricks() {
        let cases = [
            ("", PathError::Empty),
            ("../evil", PathError::DotSegment),
            ("a/../../evil", PathError::DotSegment),
            ("./a", PathError::DotSegment),
            ("/etc/passwd", PathError::Absolute),
            ("C:/Windows/x", PathError::Colon),
            ("C:\\Windows\\x", PathError::Backslash),
            ("\\\\serwer\\udział\\x", PathError::Backslash),
            ("//serwer/udział/x", PathError::Absolute),
            ("a//b", PathError::EmptySegment),
            ("a/b/", PathError::EmptySegment),
            ("plik.txt:ads", PathError::Colon),
            ("a/CON", PathError::Reserved),
            ("a/nul.txt", PathError::Reserved),
            ("a/com1", PathError::Reserved),
            ("a/COM¹.txt", PathError::Reserved),
            ("lpt0", PathError::Reserved),
            ("a/conout$.log", PathError::Reserved),
            ("a/b.", PathError::TrailingDotOrSpace),
            ("a/b ", PathError::TrailingDotOrSpace),
            ("a\u{0}b", PathError::Control),
        ];
        for (path, err) in cases {
            assert_eq!(validate_entry_path(path), Err(err), "{path:?}");
        }
        assert_eq!(
            validate_entry_path("a/".repeat(20).trim_end_matches('/')),
            Err(PathError::TooLong)
        );
        assert_eq!(
            validate_entry_path(&"x".repeat(MAX_PATH_BYTES + 1)),
            Err(PathError::TooLong)
        );
    }

    #[test]
    fn classifies_entries() {
        assert_eq!(classify("manifest.json"), EntryKind::Manifest);
        assert_eq!(
            classify("config/common/shared.toml"),
            EntryKind::Document {
                category: Category::ConfigCommon,
                name: "shared.toml".into()
            }
        );
        assert_eq!(
            classify("sessions/s1/turns.ndjson"),
            EntryKind::SessionFile {
                id: SessionId::new("s1"),
                file: "turns.ndjson".into()
            }
        );
        assert_eq!(classify("sessions/s1"), EntryKind::Unknown);
        assert_eq!(classify("config/commonx/a"), EntryKind::Unknown);
        assert_eq!(classify("inne/plik"), EntryKind::Unknown);
        assert_eq!(
            document_path(Category::Personas, "personas.json"),
            "personas/personas.json"
        );
    }
}
