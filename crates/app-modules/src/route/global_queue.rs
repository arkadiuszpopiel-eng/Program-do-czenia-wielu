//! Wspólna kolejka modeli wszystkich kont API (zgłoszenie właściciela 2026-10-14): obok kolejek
//! poszczególnych kont jedna kolejność prób dla klasy zadań, z modelami z różnych kont. Plik
//! `model-queues.json` w katalogu konfiguracji (bez sekretów — tylko identyfikatory kont i nazwy
//! modeli). Router czyta go przy każdej synchronizacji kont; zmiana działa bez restartu.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use router_contract::TaskClass;
use serde::{Deserialize, Serialize};

/// Nazwa pliku wspólnej kolejki w katalogu konfiguracji.
pub const FILE: &str = "model-queues.json";
/// Najwięcej pozycji wspólnej kolejki jednej klasy (jak kolejka konta).
pub const MAX_ENTRIES: usize = 10;

/// Pozycja wspólnej kolejki: model konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    /// Konto (`acc-…`).
    pub account_id: String,
    /// Model u dostawcy tego konta.
    pub model: String,
}

/// Klasa zadań (nazwa z interfejsu: `chat`, `code`, …) → pozycje w kolejności prób.
pub type SharedQueues = BTreeMap<String, Vec<QueueEntry>>;

/// Klasa Routera z nazwy używanej w interfejsie i w pliku.
pub fn class_of(name: &str) -> Option<TaskClass> {
    Some(match name {
        "chat" => TaskClass::Conversation,
        "code" => TaskClass::Code,
        "planning" => TaskClass::Planning,
        "voice_fast" => TaskClass::VoiceFast,
        "gui_vision" => TaskClass::GuiVision,
        "summarize" => TaskClass::Summarize,
        "embeddings" => TaskClass::Embeddings,
        _ => return None,
    })
}

/// Odczyt pliku; brak pliku albo zły format — pusta kolejka (błąd formatu w dzienniku).
pub fn load(path: &Path) -> SharedQueues {
    let Ok(text) = std::fs::read_to_string(path) else {
        return SharedQueues::new();
    };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        tracing::warn!(plik = %path.display(), "wspólna kolejka modeli nieczytelna: {e}");
        SharedQueues::new()
    })
}

/// Zapis przez plik tymczasowy i zamianę nazwy (przerwany zapis nie zostawia połowy pliku).
pub fn save(path: &Path, queues: &SharedQueues) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(queues).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Plik czytany ponownie tylko po zmianie (czas modyfikacji) — Router sprawdza go przy każdej turze.
pub(crate) struct QueueFile {
    path: PathBuf,
    cache: Mutex<(Option<SystemTime>, SharedQueues)>,
}

impl QueueFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            cache: Mutex::new((None, SharedQueues::new())),
        }
    }

    /// Bieżąca zawartość pliku.
    pub fn current(&self) -> SharedQueues {
        let modified = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if modified.is_none() {
            *cache = (None, SharedQueues::new());
        } else if cache.0 != modified {
            *cache = (modified, load(&self.path));
        }
        cache.1.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(account: &str, model: &str) -> QueueEntry {
        QueueEntry {
            account_id: account.to_owned(),
            model: model.to_owned(),
        }
    }

    #[test]
    fn saves_loads_and_follows_changes_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join(FILE);
        let file = QueueFile::new(path.clone());
        assert!(file.current().is_empty(), "brak pliku = brak kolejki");
        let queues = SharedQueues::from([(
            "chat".to_owned(),
            vec![
                entry("acc-1", "openai/gpt-6-luna"),
                entry("acc-2", "gpt-6-luna"),
            ],
        )]);
        save(&path, &queues).unwrap();
        assert_eq!(file.current(), queues);
        assert_eq!(load(&path), queues);
        std::fs::remove_file(&path).unwrap();
        assert!(file.current().is_empty(), "usunięty plik = brak kolejki");
        std::fs::write(&path, "to nie JSON").unwrap();
        assert!(load(&path).is_empty(), "zły format nie psuje Routera");
    }

    #[test]
    fn class_names_match_the_interface() {
        assert_eq!(class_of("chat"), Some(TaskClass::Conversation));
        assert_eq!(class_of("voice_fast"), Some(TaskClass::VoiceFast));
        assert_eq!(class_of("embeddings"), Some(TaskClass::Embeddings));
        assert_eq!(class_of("Rozmowa"), None);
    }
}
