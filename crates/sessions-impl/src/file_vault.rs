//! `FileKeyVault` — **NIEBEZPIECZNY w produkcji**: klucze jawnie (hex) w plikach na dysku.
//!
//! Tylko do testów i uruchomień deweloperskich bez Windows Credential Manager (feature
//! `dev-file-vault`, domyślnie wyłączony). Produkcyjny sejf: `platform-windows` (Credential
//! Manager/DPAPI). Usunięcie klucza nadpisuje plik zerami przed skasowaniem (best effort — na SSD
//! i przy kopiach systemu plików nie gwarantuje zniszczenia danych).

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use lib_sqlstore::DbKey;
use sessions_contract::{KeyVault, VaultError};

/// Sejf kluczy w plikach `<dir>/<nazwa>.key` (nazwa z `/` → `_`).
#[derive(Debug, Clone)]
pub struct FileKeyVault {
    dir: PathBuf,
}

impl FileKeyVault {
    /// Sejf w katalogu `dir` (tworzony przy pierwszym zapisie).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, name: &str) -> Result<PathBuf, VaultError> {
        let valid = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
            && !name.contains("..");
        if !valid {
            return Err(VaultError::InvalidName(name.to_owned()));
        }
        Ok(self.dir.join(format!("{}.key", name.replace('/', "_"))))
    }
}

fn io_err(e: &std::io::Error, path: &Path) -> VaultError {
    VaultError::Unavailable(format!("{}: {e}", path.display()))
}

impl KeyVault for FileKeyVault {
    fn load(&self, name: &str) -> Result<Option<DbKey>, VaultError> {
        let path = self.path(name)?;
        match std::fs::read_to_string(&path) {
            Ok(hex) => DbKey::from_hex(&hex)
                .map(Some)
                .map_err(|e| VaultError::Corrupted(e.to_string())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_err(&e, &path)),
        }
    }

    fn store(&self, name: &str, key: &DbKey) -> Result<(), VaultError> {
        let path = self.path(name)?;
        std::fs::create_dir_all(&self.dir).map_err(|e| io_err(&e, &self.dir))?;
        std::fs::write(&path, key.to_hex().as_bytes()).map_err(|e| io_err(&e, &path))
    }

    fn delete(&self, name: &str) -> Result<bool, VaultError> {
        let path = self.path(name)?;
        match std::fs::metadata(&path) {
            Ok(meta) => {
                let zeros = vec![0_u8; usize::try_from(meta.len()).unwrap_or_default()];
                let _ = std::fs::write(&path, zeros);
                std::fs::remove_file(&path).map_err(|e| io_err(&e, &path))?;
                Ok(true)
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_err(&e, &path)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_load_delete_and_reject_bad_names() {
        let dir = std::env::temp_dir().join(format!("alfa-fkv-{}", std::process::id()));
        let vault = FileKeyVault::new(&dir);
        let key = DbKey::generate().unwrap();
        vault.store("alfa/sessions/x", &key).unwrap();
        assert_eq!(vault.load("alfa/sessions/x").unwrap(), Some(key));
        assert!(vault.delete("alfa/sessions/x").unwrap());
        assert_eq!(vault.load("alfa/sessions/x").unwrap(), None);
        assert!(!vault.delete("alfa/sessions/x").unwrap());
        assert!(vault.load("../etc").is_err());
        assert!(vault.load("").is_err());
        std::fs::write(dir.join("zly.key"), "xyz").unwrap();
        assert!(matches!(vault.load("zly"), Err(VaultError::Corrupted(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
