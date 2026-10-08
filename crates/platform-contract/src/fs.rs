//! Port systemu plików z jawną odwracalnością operacji.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Operacje FS; każda ma stałą flagę odwracalności (dziennik `undo-journal`, PLAN §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsOperation {
    /// Odczyt (nie mutuje).
    Read,
    /// Zapis atomowy (tmp + rename); odwracalny — poprzednia zawartość jest zachowana.
    WriteAtomic,
    /// Kopiowanie; odwracalne — usunięcie kopii.
    Copy,
    /// Przeniesienie; odwracalne — przeniesienie z powrotem.
    Move,
    /// Usunięcie do Kosza; odwracalne — przywrócenie.
    DeleteToRecycleBin,
    /// Trwałe usunięcie; **nieodwracalne** (wymaga tokenu zdolności o wyższym poziomie).
    DeletePermanent,
}

impl FsOperation {
    /// Czy operację da się cofnąć przez `FsPort::undo`.
    pub fn is_reversible(self) -> bool {
        !matches!(self, FsOperation::DeletePermanent)
    }

    /// Czy operacja zmienia stan.
    pub fn is_mutating(self) -> bool {
        !matches!(self, FsOperation::Read)
    }
}

/// Token cofnięcia operacji (jednorazowy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UndoToken(pub u64);

/// Pokwitowanie operacji mutującej.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpReceipt {
    /// Wykonana operacja.
    pub op: FsOperation,
    /// Czy da się cofnąć (równe `op.is_reversible()`).
    pub reversible: bool,
    /// Token cofnięcia (`Some` wtedy i tylko wtedy, gdy `reversible`).
    pub undo: Option<UndoToken>,
}

impl OpReceipt {
    /// Pokwitowanie operacji odwracalnej.
    pub fn reversible(op: FsOperation, token: UndoToken) -> Self {
        Self {
            op,
            reversible: true,
            undo: Some(token),
        }
    }

    /// Pokwitowanie operacji nieodwracalnej.
    pub fn irreversible(op: FsOperation) -> Self {
        Self {
            op,
            reversible: false,
            undo: None,
        }
    }
}

/// Wpis katalogu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    /// Pełna ścieżka.
    pub path: PathBuf,
    /// Czy to katalog.
    pub is_dir: bool,
    /// Rozmiar pliku w bajtach (0 dla katalogu).
    pub size: u64,
}

/// Znane foldery (odpowiedniki `KNOWNFOLDERID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnownFolder {
    /// `%LOCALAPPDATA%`.
    LocalAppData,
    /// `%APPDATA%`.
    RoamingAppData,
    /// Katalog domowy użytkownika.
    Home,
    /// Katalog tymczasowy.
    Temp,
}

/// Fragmenty ścieżek poświadczeń, których żadna implementacja nie może czytać (AGENTS.md).
const CREDENTIAL_MARKERS: [&str; 6] = [
    ".claude",
    ".codex",
    "Credentials",
    "Login Data",
    "Cookies",
    ".ssh",
];

/// Czy ścieżka zawiera segment z deny-listy poświadczeń.
pub fn is_credential_path(path: &Path) -> bool {
    path.components().any(|c| {
        let seg = c.as_os_str().to_string_lossy();
        CREDENTIAL_MARKERS
            .iter()
            .any(|m| seg.eq_ignore_ascii_case(m))
    })
}

/// Port systemu plików. Usuwanie domyślnie do Kosza; zapis zawsze atomowy.
pub trait FsPort: Send + Sync {
    /// Odczyt całego pliku.
    fn read(&self, path: &Path) -> Result<Vec<u8>, PlatformError>;

    /// Odczyt najwyżej `max` bajtów od `offset`; zwraca fragment i rozmiar całego pliku.
    /// Domyślnie przez [`FsPort::read`]; implementacja dyskowa czyta tylko żądany zakres,
    /// żeby odczyt początku dużego pliku nie wczytywał go w całości do pamięci.
    fn read_range(
        &self,
        path: &Path,
        offset: u64,
        max: u64,
    ) -> Result<(Vec<u8>, u64), PlatformError> {
        let data = self.read(path)?;
        let total = data.len() as u64;
        let start = usize::try_from(offset.min(total)).unwrap_or(data.len());
        let end = usize::try_from(offset.saturating_add(max).min(total)).unwrap_or(data.len());
        Ok((data.get(start..end).unwrap_or_default().to_vec(), total))
    }

    /// Zapis atomowy (tmp + rename); nadpisuje istniejący plik, zachowując poprzednią wersję do cofnięcia.
    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<OpReceipt, PlatformError>;

    /// Kopiuje plik; cel nie może istnieć.
    fn copy(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError>;

    /// Przenosi plik; cel nie może istnieć.
    fn move_path(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError>;

    /// Usuwa do Kosza (odwracalne).
    fn delete_to_recycle_bin(&self, path: &Path) -> Result<OpReceipt, PlatformError>;

    /// Usuwa trwale (nieodwracalne; pokwitowanie bez tokenu).
    fn delete_permanent(&self, path: &Path) -> Result<OpReceipt, PlatformError>;

    /// Czy ścieżka istnieje.
    fn exists(&self, path: &Path) -> bool;

    /// Lista bezpośrednich wpisów katalogu.
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError>;

    /// Cofnięcie operacji po tokenie (jednorazowe).
    fn undo(&self, token: UndoToken) -> Result<(), PlatformError>;

    /// Ścieżka znanego folderu.
    fn known_folder(&self, folder: KnownFolder) -> PathBuf;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversibility_flags() {
        assert!(FsOperation::DeleteToRecycleBin.is_reversible());
        assert!(FsOperation::Move.is_reversible());
        assert!(!FsOperation::DeletePermanent.is_reversible());
        assert!(!FsOperation::Read.is_mutating());
        let r = OpReceipt::irreversible(FsOperation::DeletePermanent);
        assert!(!r.reversible && r.undo.is_none());
    }

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
}
