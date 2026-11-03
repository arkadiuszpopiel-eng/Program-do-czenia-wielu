//! Zapamiętany poziom globalny (decyzja właściciela 27.10, „Pełny dostęp” wybierany raz):
//! podniesienie globalne bez terminu, zatwierdzone przez właściciela w oknie Brokera, przetrwa
//! restart Brokera i komputera — aż właściciel sam obniży poziom. Plik leży w prywatnym katalogu
//! danych Brokera (ścieżka Jądra: agentki nie mają do niego dostępu). Zapisywany jest tylko poziom
//! powyżej domyślnego; obniżenie do domyślnego albo niżej usuwa plik (niższe poziomy odtwarza
//! dziennik rdzenia, jak dotąd).

use std::path::PathBuf;

use risk_classifier_contract::AutonomyLevel;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Standing {
    level: AutonomyLevel,
}

/// Plik zapamiętanego poziomu globalnego.
pub(crate) struct StandingStore {
    path: PathBuf,
}

impl StandingStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Zapamiętany poziom powyżej domyślnego; brak albo uszkodzony plik → `None` (poziom domyślny).
    pub(crate) fn load(&self) -> Option<AutonomyLevel> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        let saved: Standing = serde_json::from_str(&text).ok()?;
        (saved.level > AutonomyLevel::default()).then_some(saved.level)
    }

    /// Zapisuje poziom globalny bez terminu (≤ domyślny — usuwa zapis). Zapis atomowy.
    pub(crate) fn save(&self, level: AutonomyLevel) -> std::io::Result<()> {
        if level <= AutonomyLevel::default() {
            return match std::fs::remove_file(&self.path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        let text = serde_json::to_string(&Standing { level }).map_err(std::io::Error::other)?;
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
        assert_eq!(store.load(), None);
        store.save(AutonomyLevel::L4).expect("zapis L4");
        assert_eq!(store.load(), Some(AutonomyLevel::L4));
        store.save(AutonomyLevel::L2).expect("obniżenie");
        assert_eq!(store.load(), None);
        store
            .save(AutonomyLevel::L3)
            .expect("brak pliku to nie błąd");
        std::fs::write(dir.path().join("autonomy-standing.json"), "{zepsute").expect("plik");
        assert_eq!(store.load(), None, "uszkodzony zapis → poziom domyślny");
    }
}
