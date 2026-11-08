//! Biblioteka manifestów agentek z Kreatora: plik JSON na agentkę (`<persona>.json`, zapis
//! atomowy) albo pamięć (testy). Eksport w `.alfa` idzie przez `personas` (persony i role własne).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use agent_builder_contract::AgentManifest;

/// Magazyn manifestów.
pub trait ManifestStore: Send + Sync {
    /// Wszystkie zapisane manifesty.
    fn load(&self) -> Result<Vec<AgentManifest>, String>;
    /// Zapis (zastępuje manifest tej samej persony).
    fn save(&self, manifest: &AgentManifest) -> Result<(), String>;
}

/// Magazyn w pamięci.
#[derive(Debug, Default)]
pub struct MemManifestStore {
    inner: Mutex<Vec<AgentManifest>>,
}

impl MemManifestStore {
    fn lock(&self) -> MutexGuard<'_, Vec<AgentManifest>> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl ManifestStore for MemManifestStore {
    fn load(&self) -> Result<Vec<AgentManifest>, String> {
        Ok(self.lock().clone())
    }

    fn save(&self, manifest: &AgentManifest) -> Result<(), String> {
        let mut v = self.lock();
        v.retain(|m| m.persona.id != manifest.persona.id);
        v.push(manifest.clone());
        Ok(())
    }
}

/// Magazyn katalogowy.
#[derive(Debug, Clone)]
pub struct DirManifestStore {
    dir: PathBuf,
}

impl DirManifestStore {
    /// Otwiera (tworzy) katalog.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, String> {
        fs::create_dir_all(dir.as_ref()).map_err(|e| e.to_string())?;
        Ok(Self {
            dir: dir.as_ref().to_path_buf(),
        })
    }
}

impl ManifestStore for DirManifestStore {
    fn load(&self) -> Result<Vec<AgentManifest>, String> {
        let mut names: Vec<PathBuf> = fs::read_dir(&self.dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        names.sort();
        names
            .iter()
            .map(|p| {
                let data = fs::read(p).map_err(|e| e.to_string())?;
                serde_json::from_slice(&data).map_err(|e| format!("{}: {e}", p.display()))
            })
            .collect()
    }

    fn save(&self, manifest: &AgentManifest) -> Result<(), String> {
        // Identyfikator persony jest zwalidowany (`[a-z][a-z0-9-]`), więc bezpieczny jako nazwa pliku.
        let path = self.dir.join(format!("{}.json", manifest.persona.id));
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&data).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }
}
