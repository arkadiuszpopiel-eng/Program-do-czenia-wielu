//! Magazyn wersji umiejętności: plik JSON w katalogu modułu (zapis atomowy: plik tymczasowy +
//! `rename`; urwany zapis zostawia poprzedni stan) albo pamięć (testy).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use skills_contract::SkillRecord;

/// Magazyn wersji.
pub trait SkillStore: Send + Sync {
    /// Wszystkie zapisane wersje.
    fn load(&self) -> Result<Vec<SkillRecord>, String>;
    /// Zapis całości (atomowo).
    fn save(&self, records: &[SkillRecord]) -> Result<(), String>;
}

/// Magazyn w pamięci.
#[derive(Debug, Default)]
pub struct MemSkillStore {
    inner: Mutex<Vec<SkillRecord>>,
}

impl MemSkillStore {
    fn lock(&self) -> MutexGuard<'_, Vec<SkillRecord>> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl SkillStore for MemSkillStore {
    fn load(&self) -> Result<Vec<SkillRecord>, String> {
        Ok(self.lock().clone())
    }

    fn save(&self, records: &[SkillRecord]) -> Result<(), String> {
        *self.lock() = records.to_vec();
        Ok(())
    }
}

/// Magazyn plikowy (`<katalog>/skills.json`).
#[derive(Debug, Clone)]
pub struct DirSkillStore {
    path: PathBuf,
}

impl DirSkillStore {
    /// Otwiera (tworzy) katalog.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, String> {
        fs::create_dir_all(dir.as_ref()).map_err(|e| e.to_string())?;
        Ok(Self {
            path: dir.as_ref().join("skills.json"),
        })
    }

    /// Ścieżka pliku.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SkillStore for DirSkillStore {
    fn load(&self) -> Result<Vec<SkillRecord>, String> {
        match fs::read(&self.path) {
            Ok(data) => serde_json::from_slice(&data).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, records: &[SkillRecord]) -> Result<(), String> {
        let tmp = self.path.with_extension("json.tmp");
        let data = serde_json::to_vec_pretty(records).map_err(|e| e.to_string())?;
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&data).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}
