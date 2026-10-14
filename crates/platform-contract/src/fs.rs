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

/// Czy ścieżka zawiera segment z deny-listy poświadczeń — także zapisany wzorcem (`.s?h`,
/// `.cl*`, `[.]ssh`), który może go objąć (audyt 2026-10-14, A-07).
pub fn is_credential_path(path: &Path) -> bool {
    path.components().any(|c| {
        let seg = c.as_os_str().to_string_lossy();
        CREDENTIAL_MARKERS
            .iter()
            .any(|m| seg.eq_ignore_ascii_case(m) || glob_matches(&seg, m))
    })
}

/// Czy wzorzec z `*`, `?` i `[…]` może pasować do nazwy (bez rozróżniania wielkości liter).
/// `[…]` traktowany jak dowolny znak — ostrożnie, w stronę „pasuje”.
fn glob_matches(pattern: &str, name: &str) -> bool {
    if !pattern.contains(['*', '?', '[']) {
        return false;
    }
    let mut pat: Vec<char> = Vec::new();
    let mut in_class = false;
    for c in pattern.to_lowercase().chars() {
        match (c, in_class) {
            ('[', false) => {
                in_class = true;
                pat.push('?');
            }
            (']', true) => in_class = false,
            (_, true) => {}
            (c, false) => pat.push(c),
        }
    }
    let text: Vec<char> = name.to_lowercase().chars().collect();
    // Dopasowanie z nawrotem dla `*` (wzorce są krótkie — segment ścieżki).
    let (mut p, mut t, mut star, mut mark) = (0, 0, None, 0);
    while t < text.len() {
        if p < pat.len() && (pat[p] == '?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == '*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    pat[p..].iter().all(|c| *c == '*')
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
}
