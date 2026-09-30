//! Przeładowanie na żywo: obserwator plików (`notify`) z debounce → `FileConfigStore::reload`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use core_config_contract::ConfigError;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;

use crate::store::FileConfigStore;

/// Domyślne okno debounce (SPEC: reakcja na zmianę pliku ≤ 200 ms).
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(100);

/// Aktywna obserwacja; porzucenie uchwytu zatrzymuje obserwatora i zadanie.
pub struct FileWatch {
    _watcher: RecommendedWatcher,
    task: JoinHandle<()>,
}

impl Drop for FileWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Czy zmiana dotyczy pliku warstwy (`*.toml`, bez plików tymczasowych zapisu atomowego).
fn is_layer_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "toml")
}

/// Obserwuje katalog konfiguracji; seria zdarzeń w oknie `debounce` daje jedno `reload()`.
/// Wymaga działającego runtime tokio. Błędy przeładowania trafiają do `config.invalid`.
pub fn watch_files(
    store: Arc<FileConfigStore>,
    debounce: Duration,
) -> Result<FileWatch, ConfigError> {
    let (tx, mut rx) = unbounded_channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res
            && event.paths.iter().any(|p| is_layer_file(p))
        {
            let _ = tx.send(());
        }
    })
    .map_err(|e| ConfigError::Persist(format!("obserwator plików: {e}")))?;
    watcher
        .watch(store.dir(), RecursiveMode::Recursive)
        .map_err(|e| ConfigError::Persist(format!("obserwator plików: {e}")))?;
    let task = tokio::spawn(async move {
        while rx.recv().await.is_some() {
            loop {
                match tokio::time::timeout(debounce, rx.recv()).await {
                    Ok(Some(())) => {}
                    Ok(None) => return,
                    Err(_) => break,
                }
            }
            if let Err(e) = store.reload().await {
                tracing::warn!(error = %e, "przeładowanie konfiguracji odrzucone");
            }
        }
    });
    Ok(FileWatch {
        _watcher: watcher,
        task,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_layer_files() {
        assert!(is_layer_file(Path::new("c/shared.toml")));
        assert!(is_layer_file(Path::new("c/machine/m1.toml")));
        assert!(!is_layer_file(Path::new("c/shared.toml.tmp-42")));
        assert!(!is_layer_file(Path::new("c/history.ndjson")));
    }
}
