//! Testy publicznego API `fs.rs`: domyślny odczyt zakresu i rozpoznawanie ścieżek poświadczeń
//! (także zapisanych wzorcem — audyt 2026-10-14, A-07).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use platform_contract::{
    DirEntry, FsPort, KnownFolder, OpReceipt, PlatformError, UndoToken, is_credential_path,
};

struct OneFile(Vec<u8>);

impl FsPort for OneFile {
    fn read(&self, _: &Path) -> Result<Vec<u8>, PlatformError> {
        Ok(self.0.clone())
    }
    fn write_atomic(&self, p: &Path, _: &[u8]) -> Result<OpReceipt, PlatformError> {
        Err(PlatformError::InvalidPath(p.to_path_buf()))
    }
    fn copy(&self, p: &Path, _: &Path) -> Result<OpReceipt, PlatformError> {
        Err(PlatformError::InvalidPath(p.to_path_buf()))
    }
    fn move_path(&self, p: &Path, _: &Path) -> Result<OpReceipt, PlatformError> {
        Err(PlatformError::InvalidPath(p.to_path_buf()))
    }
    fn delete_to_recycle_bin(&self, p: &Path) -> Result<OpReceipt, PlatformError> {
        Err(PlatformError::InvalidPath(p.to_path_buf()))
    }
    fn delete_permanent(&self, p: &Path) -> Result<OpReceipt, PlatformError> {
        Err(PlatformError::InvalidPath(p.to_path_buf()))
    }
    fn exists(&self, _: &Path) -> bool {
        true
    }
    fn list_dir(&self, _: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        Ok(Vec::new())
    }
    fn undo(&self, _: UndoToken) -> Result<(), PlatformError> {
        Ok(())
    }
    fn known_folder(&self, _: KnownFolder) -> PathBuf {
        PathBuf::new()
    }
}

#[test]
fn default_read_range_slices_and_reports_total() {
    let fs = OneFile(b"0123456789".to_vec());
    let p = Path::new("plik");
    assert_eq!(fs.read_range(p, 0, 4).ok(), Some((b"0123".to_vec(), 10)));
    assert_eq!(fs.read_range(p, 8, 100).ok(), Some((b"89".to_vec(), 10)));
    assert_eq!(fs.read_range(p, 50, 4).ok(), Some((Vec::new(), 10)));
    assert_eq!(
        fs.read_range(p, 3, u64::MAX).map(|r| r.0.len()).ok(),
        Some(7)
    );
}

#[test]
fn credential_paths_detected() {
    assert!(is_credential_path(Path::new("/home/u/.claude/creds.json")));
    assert!(is_credential_path(Path::new(
        "C:/Users/u/AppData/Local/Google/Chrome/User Data/Default/Login Data"
    )));
    assert!(!is_credential_path(Path::new(
        "/home/u/projects/claude-notes.md"
    )));
}

/// Audyt A-07: wzorzec, który może objąć katalog poświadczeń, liczy się jak ten katalog.
#[test]
fn credential_globs_detected() {
    for p in [
        "C:/Users/a/.s?h/id_rsa",
        "C:/Users/a/.SS*/id_rsa",
        "C:/Users/a/[.]ssh/id_rsa",
        "C:/Users/a/*/x",
        "/home/u/.cl*de/creds.json",
        "C:/Chrome/User Data/Default/Login*",
    ] {
        assert!(is_credential_path(Path::new(p)), "{p}");
    }
    for p in ["C:/Users/a/*.txt", "C:/Users/a/do?s/x", "/home/u/.s?x/a"] {
        assert!(!is_credential_path(Path::new(p)), "{p}");
    }
}
