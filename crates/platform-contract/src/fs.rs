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
