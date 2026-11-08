//! Magazyn wtyczek w katalogu (`%LOCALAPPDATA%\Alfa\plugins`): `plugins.json` (rekordy,
//! zapis atomowy przez plik tymczasowy) i `wasm\<sha256>.wasm` (nazwa wyłącznie z hasha —
//! bez ścieżek z zewnątrz). Bajty są i tak weryfikowane hashem przy każdym ładowaniu.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use plugin_runtime_contract::{PluginRecord, PluginStore, wasm_key};

/// Magazyn w katalogu.
#[derive(Debug, Clone)]
pub struct DirPluginStore {
    root: PathBuf,
}

const RECORDS: &str = "plugins.json";
const WASM_DIR: &str = "wasm";

fn io(e: std::io::Error) -> String {
    e.to_string()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    let mut f = fs::File::create(&tmp).map_err(io)?;
    f.write_all(bytes).map_err(io)?;
    f.sync_all().map_err(io)?;
    drop(f);
    fs::rename(&tmp, path).map_err(io)
}

impl DirPluginStore {
    /// Otwiera (tworzy) katalog magazynu.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        fs::create_dir_all(root.join(WASM_DIR)).map_err(io)?;
        Ok(Self { root })
    }

    fn wasm_path(&self, sha256: &str) -> Result<PathBuf, String> {
        Ok(self
            .root
            .join(WASM_DIR)
            .join(format!("{}.wasm", wasm_key(sha256)?)))
    }
}

impl PluginStore for DirPluginStore {
    fn load_records(&self) -> Result<Vec<PluginRecord>, String> {
        match fs::read(self.root.join(RECORDS)) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(io(e)),
        }
    }

    fn save_records(&self, records: &[PluginRecord]) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(records).map_err(|e| e.to_string())?;
        write_atomic(&self.root.join(RECORDS), &bytes)
    }

    fn put_wasm(&self, sha256: &str, bytes: &[u8]) -> Result<(), String> {
        write_atomic(&self.wasm_path(sha256)?, bytes)
    }

    fn get_wasm(&self, sha256: &str) -> Result<Option<Vec<u8>>, String> {
        match fs::read(self.wasm_path(sha256)?) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(e)),
        }
    }

    fn delete_wasm(&self, sha256: &str) -> Result<(), String> {
        match fs::remove_file(self.wasm_path(sha256)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(e)),
        }
    }
}
