//! Weryfikacja integralności zestawu: SHA-256 każdego pliku względem manifestu.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::EvalError;
use crate::manifest::{SuiteId, SuiteManifest, SuiteStatus};

/// SHA-256 jako hex małymi literami (jak `sha256sum`).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Czy tekst to SHA-256 hex (64 znaki `[0-9a-f]`).
pub fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Plik o innej treści niż w manifeście.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FileMismatch {
    /// Ścieżka.
    pub path: String,
    /// Hash z manifestu.
    pub expected: String,
    /// Hash bieżący.
    pub actual: String,
}

/// Wynik weryfikacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrityReport {
    /// Zestaw.
    pub suite: SuiteId,
    /// Status zestawu.
    pub status: SuiteStatus,
    /// Hash manifestu.
    pub manifest_digest: String,
    /// Liczba sprawdzonych plików.
    pub checked: usize,
    /// Pliki zmienione.
    pub mismatched: Vec<FileMismatch>,
    /// Pliki brakujące.
    pub missing: Vec<String>,
}

impl IntegrityReport {
    /// Czy wszystkie pliki są zgodne.
    pub fn is_intact(&self) -> bool {
        self.mismatched.is_empty() && self.missing.is_empty()
    }

    /// Błąd dla zestawu zamrożonego (albo `strict`, np. holdout) z naruszoną integralnością.
    /// Zestaw `proposed` tylko raportuje rozjazd (ACCEPTANCE §1: zamraża człowiek).
    pub fn enforce(&self, strict: bool) -> Result<(), EvalError> {
        if self.is_intact() || !(strict || self.status == SuiteStatus::Frozen) {
            return Ok(());
        }
        let mut parts: Vec<String> = self
            .mismatched
            .iter()
            .map(|m| format!("zmieniony `{}`", m.path))
            .collect();
        parts.extend(self.missing.iter().map(|p| format!("brak `{p}`")));
        Err(EvalError::IntegrityViolation {
            suite: self.suite.to_string(),
            detail: parts.join(", "),
        })
    }
}

/// Weryfikuje pliki manifestu; `read` zwraca treść pliku albo `None`, gdy go nie ma.
pub fn verify_files<F>(manifest: &SuiteManifest, mut read: F) -> IntegrityReport
where
    F: FnMut(&str) -> Option<Vec<u8>>,
{
    let mut report = IntegrityReport {
        suite: manifest.suite.clone(),
        status: manifest.status,
        manifest_digest: manifest.digest(),
        checked: 0,
        mismatched: Vec::new(),
        missing: Vec::new(),
    };
    for (path, expected) in &manifest.files {
        report.checked += 1;
        match read(path) {
            None => report.missing.push(path.clone()),
            Some(bytes) => {
                let actual = sha256_hex(&bytes);
                if actual != *expected {
                    report.mismatched.push(FileMismatch {
                        path: path.clone(),
                        expected: expected.clone(),
                        actual,
                    });
                }
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::manifest::MANIFEST_SCHEMA_VERSION;

    #[test]
    fn known_vector_and_hex_check() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(is_sha256_hex(&sha256_hex(b"")));
        assert!(!is_sha256_hex(&"A".repeat(64)));
        assert!(!is_sha256_hex("ab"));
    }

    #[test]
    fn frozen_change_is_error_proposed_is_drift() {
        let files = BTreeMap::from([
            ("a".to_owned(), sha256_hex(b"1")),
            ("b".to_owned(), sha256_hex(b"2")),
        ]);
        let mut m = SuiteManifest {
            schema: MANIFEST_SCHEMA_VERSION,
            suite: SuiteId::new("s").unwrap(),
            wave: "F8".into(),
            version: 1,
            status: SuiteStatus::Frozen,
            created: "2026-10-01".into(),
            accepted_by: Some("właściciel".into()),
            description: String::new(),
            files,
            cases: Vec::new(),
            thresholds: Vec::new(),
        };
        let read = |p: &str| match p {
            "a" => Some(b"1".to_vec()),
            "b" => Some(b"zmienione".to_vec()),
            _ => None,
        };
        let r = verify_files(&m, read);
        assert_eq!(r.checked, 2);
        assert_eq!(r.mismatched.len(), 1);
        assert!(matches!(
            r.enforce(false),
            Err(EvalError::IntegrityViolation { .. })
        ));
        m.status = SuiteStatus::Proposed;
        let r = verify_files(&m, |_| None);
        assert_eq!(r.missing.len(), 2);
        assert!(r.enforce(false).is_ok());
        assert!(r.enforce(true).is_err());
    }
}
