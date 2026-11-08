//! Zapamiętany poziom globalny (decyzja właściciela 27.10, „Pełny dostęp” wybierany raz):
//! podniesienie globalne bez terminu, zatwierdzone przez właściciela w oknie Brokera, przetrwa
//! restart Brokera i komputera — aż właściciel sam obniży poziom. Tak samo przełącznik „Max dostęp”
//! (plan 50 §8.1). Plik leży w prywatnym katalogu danych Brokera (ścieżka Jądra: agentki nie mają
//! do niego dostępu). Zapisywany jest tylko poziom powyżej domyślnego i włączony „Max dostęp”;
//! gdy nie ma żadnego z nich, plik jest usuwany (niższe poziomy odtwarza dziennik rdzenia).

use std::path::PathBuf;

use risk_classifier_contract::AutonomyLevel;
use safety_broker_contract::ModelScope;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Standing {
    level: AutonomyLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_access: Option<ModelScope>,
}

/// Zapamiętany stan: poziom globalny powyżej domyślnego i „Max dostęp”.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Remembered {
    /// Poziom globalny powyżej domyślnego.
    pub(crate) level: Option<AutonomyLevel>,
    /// Włączony „Max dostęp”.
    pub(crate) max_access: Option<ModelScope>,
}

/// Plik zapamiętanego poziomu globalnego.
pub(crate) struct StandingStore {
    path: PathBuf,
}

impl StandingStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Zapamiętany stan; brak albo uszkodzony plik → nic (poziom domyślny, „Max dostęp” wyłączony).
    pub(crate) fn load(&self) -> Remembered {
        let Some(saved) = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Standing>(&text).ok())
        else {
            return Remembered::default();
        };
        Remembered {
            level: (saved.level > AutonomyLevel::default()).then_some(saved.level),
            max_access: saved.max_access,
        }
    }

    /// Zapisuje poziom globalny bez terminu (≤ domyślny — bez poziomu); „Max dostęp” zostaje.
    pub(crate) fn save(&self, level: AutonomyLevel) -> std::io::Result<()> {
        let mut now = self.load();
        now.level = (level > AutonomyLevel::default()).then_some(level);
        self.write(now)
    }

    /// Zapisuje „Max dostęp” (`None` — wyłączony); poziom globalny zostaje.
    pub(crate) fn save_max_access(&self, scope: Option<ModelScope>) -> std::io::Result<()> {
        let mut now = self.load();
        now.max_access = scope;
        self.write(now)
    }

    fn write(&self, state: Remembered) -> std::io::Result<()> {
        if state == Remembered::default() {
            return match std::fs::remove_file(&self.path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        let saved = Standing {
            level: state.level.unwrap_or_default(),
            max_access: state.max_access,
        };
        let text = serde_json::to_string(&saved).map_err(std::io::Error::other)?;
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_levels_above_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = StandingStore::new(dir.path().join("autonomy-standing.json"));
        assert_eq!(store.load(), Remembered::default());
        store.save(AutonomyLevel::L4).expect("zapis L4");
        assert_eq!(store.load().level, Some(AutonomyLevel::L4));
        store.save(AutonomyLevel::L2).expect("obniżenie");
        assert_eq!(store.load(), Remembered::default());
        assert!(!dir.path().join("autonomy-standing.json").exists());
        store
            .save(AutonomyLevel::L3)
            .expect("brak pliku to nie błąd");
        std::fs::write(dir.path().join("autonomy-standing.json"), "{zepsute").expect("plik");
        assert_eq!(
            store.load(),
            Remembered::default(),
            "uszkodzony zapis → poziom domyślny"
        );
    }

    #[test]
    fn max_access_is_kept_next_to_the_level() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("autonomy-standing.json");
        let store = StandingStore::new(path.clone());
        store
            .save_max_access(Some(ModelScope::Local))
            .expect("zapis Max");
        assert_eq!(store.load().max_access, Some(ModelScope::Local));
        assert_eq!(store.load().level, None);
        store.save(AutonomyLevel::L4).expect("zapis L4");
        assert_eq!(
            store.load(),
            Remembered {
                level: Some(AutonomyLevel::L4),
                max_access: Some(ModelScope::Local)
            }
        );
        store.save(AutonomyLevel::L1).expect("obniżenie");
        assert_eq!(
            store.load().max_access,
            Some(ModelScope::Local),
            "Max zostaje"
        );
        store.save_max_access(None).expect("wyłączenie");
        assert!(!path.exists());
        // Plik ze starszej wersji (bez pola) czyta się jak dotąd.
        std::fs::write(&path, r#"{"level":"L4"}"#).expect("plik");
        assert_eq!(store.load().level, Some(AutonomyLevel::L4));
    }
}
