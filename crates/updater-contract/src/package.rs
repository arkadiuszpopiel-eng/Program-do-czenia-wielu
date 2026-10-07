//! Paczka aktualizacji (ZIP): układ i reguły rozpakowania wspólne dla `-impl` i testów —
//! ścieżki wpisów (ochrona przed path traversal, strumieniami NTFS, nazwami urządzeń),
//! limity zip-bomb (liczba wpisów, rozmiar wpisu i całości, stopień kompresji).
//!
//! Układ paczki (katalog główny archiwum = katalog `versions\<ver>\`):
//! `alfa-desktop.exe` (wymagany), `version.json` (wymagany, `{ "version": "<ver>" }`),
//! `notes.md` („Co nowego”, opcjonalny), `alfa.exe` (nowy launcher, opcjonalny — zamieniany
//! przy następnym starcie po `mark_good`), dowolne zasoby wersji.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::UpdaterError;

/// Notatki „Co nowego” w katalogu wersji (z podpisanej paczki).
pub const NOTES_FILE: &str = "notes.md";

/// Maksymalna długość ścieżki wpisu (bajty).
pub const MAX_PATH_BYTES: usize = 240;
/// Maksymalna głębokość ścieżki wpisu.
pub const MAX_DEPTH: usize = 16;

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM1", "COM2", "COM3", "COM4", "COM5",
    "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
    "LPT9",
];

/// Limity rozpakowania paczki aktualizacji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageLimits {
    /// Maksymalna liczba wpisów.
    pub max_entries: u64,
    /// Maksymalny rozmiar wpisu po rozpakowaniu.
    pub max_entry_bytes: u64,
    /// Maksymalny łączny rozmiar po rozpakowaniu.
    pub max_total_bytes: u64,
    /// Maksymalny stopień kompresji (rozpakowany / spakowany) dla wpisów > 1 MiB.
    pub max_ratio: u64,
}

impl Default for PackageLimits {
    fn default() -> Self {
        Self {
            max_entries: 4_096,
            max_entry_bytes: 512 << 20,
            max_total_bytes: 1_536 << 20,
            max_ratio: 200,
        }
    }
}

impl PackageLimits {
    /// Sprawdza wpis wg rozmiarów z nagłówka; zwraca nową sumę rozpakowanych bajtów.
    pub fn admit(&self, size: u64, compressed: u64, total: u64) -> Result<u64, UpdaterError> {
        if size > self.max_entry_bytes {
            return Err(UpdaterError::unsafe_package(format!(
                "wpis {size} B > {} B",
                self.max_entry_bytes
            )));
        }
        if size > 1 << 20 && size / compressed.max(1) > self.max_ratio {
            return Err(UpdaterError::unsafe_package(
                "podejrzany stopień kompresji (zip-bomb)",
            ));
        }
        let total = total.saturating_add(size);
        if total > self.max_total_bytes {
            return Err(UpdaterError::unsafe_package(format!(
                "łącznie {total} B > {} B",
                self.max_total_bytes
            )));
        }
        Ok(total)
    }
}

/// Ścieżka wpisu: względna, `/` jako separator, bez `.`/`..`, bez `\`, `:` (dyski, strumienie
/// NTFS), znaków sterujących i zarezerwowanych w Windows, bez nazw urządzeń, bez kropki/spacji
/// na końcu segmentu. Katalog kończy się `/` (dozwolone).
pub fn validate_package_path(name: &str) -> Result<(), UpdaterError> {
    let bad = |why: &str| {
        Err(UpdaterError::unsafe_package(format!(
            "ścieżka „{name}”: {why}"
        )))
    };
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    if trimmed.is_empty() {
        return bad("pusta");
    }
    if name.len() > MAX_PATH_BYTES {
        return bad("za długa");
    }
    if name.starts_with('/') {
        return bad("bezwzględna");
    }
    if name
        .chars()
        .any(|c| c.is_control() || matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return bad("niedozwolony znak");
    }
    let segments: Vec<&str> = trimmed.split('/').collect();
    if segments.len() > MAX_DEPTH {
        return bad("za głęboka");
    }
    for segment in segments {
        if segment.is_empty() || segment == "." || segment == ".." {
            return bad("segment pusty albo „..”");
        }
        if segment.ends_with('.') || segment.ends_with(' ') {
            return bad("kropka albo spacja na końcu");
        }
        let stem = segment.split('.').next().unwrap_or(segment).trim_end();
        if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
            return bad("nazwa urządzenia");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_and_windows_tricks_are_rejected() {
        for ok in [
            "alfa-desktop.exe",
            "version.json",
            "zasoby/ikony/a.png",
            "zasoby/",
            "notes.md",
        ] {
            assert!(validate_package_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "/",
            "../alfa.exe",
            "a/../../b",
            "/etc/passwd",
            "C:/Windows/x.dll",
            "C:x",
            "a\\..\\b",
            "alfa.exe:zone",
            "a//b",
            "./a",
            "NUL",
            "con.txt",
            "Com1.dll",
            "zasoby/aux",
            "a. ",
            "a.",
            "a\u{0}b",
            "a?b",
        ] {
            assert!(
                matches!(
                    validate_package_path(bad),
                    Err(UpdaterError::UnsafePackage { .. })
                ),
                "{bad:?}"
            );
        }
        let deep = vec!["a"; MAX_DEPTH + 1].join("/");
        assert!(validate_package_path(&deep).is_err());
        assert!(validate_package_path(&"a".repeat(MAX_PATH_BYTES + 1)).is_err());
    }

    #[test]
    fn limits_stop_zip_bombs() {
        let l = PackageLimits {
            max_entries: 10,
            max_entry_bytes: 10 << 20,
            max_total_bytes: 12 << 20,
            max_ratio: 100,
        };
        assert_eq!(
            l.admit(100, 1, 0).unwrap(),
            100,
            "małe wpisy bez progu kompresji"
        );
        assert!(l.admit(11 << 20, 11 << 20, 0).is_err(), "za duży wpis");
        assert!(l.admit(5 << 20, 1 << 10, 0).is_err(), "kompresja 5120:1");
        let total = l.admit(6 << 20, 6 << 20, 0).unwrap();
        assert!(l.admit(7 << 20, 7 << 20, total).is_err(), "łącznie za dużo");
        assert!(l.admit(u64::MAX, u64::MAX, u64::MAX).is_err());
    }
}
