//! Obserwacja katalogów wyzwalaczy plikowych na porcie platformy (`DirWatchPort`:
//! `ReadDirectoryChangesW`, debounce, deny-lista, przeskanowanie po przepełnieniu) — adapter
//! `FileWatchPort` wyzwalaczy i pompa „nowy plik" → `TriggersModule::file_created`.

use std::sync::{Arc, Weak};
use std::time::Duration;

use platform_contract::{DirWatchPort, WatchSpec};
use triggers_impl::{FileWatchPort, TriggersModule};

/// Adapter: zbiór katalogów wyzwalaczy → obserwacje platformy (zastępuje poprzedni zbiór).
pub struct PlatformFileWatch {
    port: Arc<dyn DirWatchPort>,
}

impl PlatformFileWatch {
    /// Adapter na porcie platformy.
    pub fn new(port: Arc<dyn DirWatchPort>) -> Self {
        Self { port }
    }
}

impl FileWatchPort for PlatformFileWatch {
    fn watch(&self, dirs: Vec<String>) {
        let specs = dirs.into_iter().map(WatchSpec::new).collect();
        for result in self.port.replace_all(specs) {
            if let Err(e) = result {
                // Katalog spoza dozwolonych (deny-lista) albo niedostępny — bez treści ścieżki.
                tracing::warn!(error = %e, "obserwacja katalogu wyzwalacza odrzucona");
            }
        }
    }
}

/// Pompa zdarzeń (wątek blokujący): nowe pliki → wyzwalacze, dopóki moduł żyje.
pub fn spawn_pump(port: Arc<dyn DirWatchPort>, triggers: Weak<TriggersModule>) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    handle.spawn_blocking(move || {
        loop {
            let events = port.wait_events(Duration::from_secs(1));
            let Some(module) = triggers.upgrade() else {
                return;
            };
            if events.is_empty() {
                // Port bez blokującego czekania (atrapa) — bez pętli na pusto.
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            for event in &events {
                if let Some(path) = event.new_file() {
                    module.file_created(&path.to_string_lossy());
                }
            }
        }
    });
}
