//! Magazyny kotwicy głowy łańcucha Audytu. Produkcyjnie plik w katalogu konta usługi
//! Brokera (ACL — część 2; TPM rozważany w ADR z THREAT_MODEL §11).

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use safety_broker_contract::{AnchorStore, ChainAnchor};

/// Kotwica w pamięci (testy, atrapa).
#[derive(Debug, Default)]
pub struct MemoryAnchorStore {
    anchor: Mutex<Option<ChainAnchor>>,
}

impl MemoryAnchorStore {
    fn lock(&self) -> MutexGuard<'_, Option<ChainAnchor>> {
        self.anchor.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl AnchorStore for MemoryAnchorStore {
    fn load(&self) -> Result<Option<ChainAnchor>, String> {
        Ok(self.lock().clone())
    }

    fn store(&self, anchor: &ChainAnchor) -> Result<(), String> {
        *self.lock() = Some(anchor.clone());
        Ok(())
    }
}

/// Kotwica w pliku JSON, zapis atomowy (plik tymczasowy + `sync_all` + `rename`).
#[derive(Debug)]
pub struct FileAnchorStore {
    path: PathBuf,
}

impl FileAnchorStore {
    /// Kotwica w podanym pliku (katalog nadrzędny tworzony przy zapisie).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl AnchorStore for FileAnchorStore {
    fn load(&self) -> Result<Option<ChainAnchor>, String> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| format!("kotwica uszkodzona: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("odczyt kotwicy: {e}")),
        }
    }

    fn store(&self, anchor: &ChainAnchor) -> Result<(), String> {
        let io = |what: &str, e: std::io::Error| format!("{what}: {e}");
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| io("katalog kotwicy", e))?;
        }
        let tmp = self.path.with_extension("tmp");
        let body = serde_json::to_vec(anchor).map_err(|e| format!("serializacja kotwicy: {e}"))?;
        let mut f = fs::File::create(&tmp).map_err(|e| io("plik tymczasowy kotwicy", e))?;
        f.write_all(&body)
            .and_then(|()| f.sync_all())
            .map_err(|e| io("zapis kotwicy", e))?;
        fs::rename(&tmp, &self.path).map_err(|e| io("podmiana kotwicy", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_anchor_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileAnchorStore::new(dir.path().join("k").join("anchor.json"));
        assert_eq!(store.load().unwrap(), None);
        let a = ChainAnchor {
            chain_id: "c".into(),
            records: 3,
            head: "ab".into(),
        };
        store.store(&a).unwrap();
        assert_eq!(store.load().unwrap(), Some(a));
        fs::write(dir.path().join("k").join("anchor.json"), b"{zepsute").unwrap();
        assert!(store.load().is_err());
        let mem = MemoryAnchorStore::default();
        assert_eq!(mem.load().unwrap(), None);
    }
}
