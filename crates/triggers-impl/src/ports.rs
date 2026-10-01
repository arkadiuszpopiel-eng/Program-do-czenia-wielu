//! Porty: magazyn stanu wyzwalaczy i obserwacja katalogów (platforma).

use std::path::PathBuf;
use std::sync::Mutex;

use triggers_contract::TriggerSnapshot;

/// Magazyn stanu wyzwalaczy.
pub trait TriggerStore: Send + Sync {
    /// Ostatni zapisany stan.
    fn load(&self) -> Result<Option<TriggerSnapshot>, String>;
    /// Zapis atomowy.
    fn save(&self, snapshot: &TriggerSnapshot) -> Result<(), String>;
}

/// Magazyn w pamięci.
#[derive(Debug, Default)]
pub struct MemTriggerStore(Mutex<Option<TriggerSnapshot>>);

impl TriggerStore for MemTriggerStore {
    fn load(&self) -> Result<Option<TriggerSnapshot>, String> {
        Ok(self.0.lock().unwrap_or_else(|p| p.into_inner()).clone())
    }

    fn save(&self, snapshot: &TriggerSnapshot) -> Result<(), String> {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = Some(snapshot.clone());
        Ok(())
    }
}

/// Stan w pliku JSON (`%LOCALAPPDATA%\Alfa\triggers\state.json`), zapis przez plik tymczasowy;
/// zapisy szeregowane.
#[derive(Debug)]
pub struct FileTriggerStore {
    path: PathBuf,
    write: Mutex<()>,
}

impl FileTriggerStore {
    /// Magazyn w pliku `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write: Mutex::new(()),
        }
    }
}

impl TriggerStore for FileTriggerStore {
    fn load(&self) -> Result<Option<TriggerSnapshot>, String> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| format!("uszkodzony stan wyzwalaczy: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("odczyt stanu wyzwalaczy: {e}")),
        }
    }

    fn save(&self, snapshot: &TriggerSnapshot) -> Result<(), String> {
        let _guard = self.write.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("katalog stanu: {e}"))?;
        }
        let bytes = serde_json::to_vec(snapshot).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| format!("zapis stanu: {e}"))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| format!("zapis stanu: {e}"))
    }
}

/// Port obserwacji katalogów (powłoka podpina `ReadDirectoryChangesW` z `platform-windows`
/// i woła `TriggersModule::file_created`).
pub trait FileWatchPort: Send + Sync {
    /// Nowy zbiór obserwowanych katalogów (zastępuje poprzedni).
    fn watch(&self, dirs: Vec<String>);
}

/// Brak obserwacji (wyzwalacze plikowe działają tylko przez `file_created`).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoFileWatch;

impl FileWatchPort for NoFileWatch {
    fn watch(&self, _dirs: Vec<String>) {}
}
