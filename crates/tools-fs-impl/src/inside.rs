//! Zawartość katalogu przed jego usunięciem albo przeniesieniem (plan 50 §8, luka 3 z przeglądu
//! 3.11). Sędzia ocenia ścieżki leksykalnie, więc usunięcie katalogu z podrepozytorium (`.git`)
//! albo z `.vscode` nie było „zmianą ścieżki chronionej”. Narzędzie przegląda drzewo na dysku
//! i zgłasza Sędziemu fakt `contains_protected`; przy zbyt dużym albo nieczytelnym drzewie — też
//! (ostrożnie: zapyta).

use std::collections::VecDeque;
use std::path::Path;

use safety_broker_contract::{PathScope, is_protected_in};

use crate::core::Core;

/// Najwięcej wpisów przeglądanych przed uznaniem drzewa za „może zawierać”.
const MAX_ENTRIES: usize = 5_000;

impl Core {
    /// Czy katalog `path` zawiera ścieżkę chronioną (plik albo brak — `false`).
    pub(crate) fn holds_protected(&self, path: &str, workdir: Option<&str>) -> bool {
        let Ok(first) = self.fs.list_dir(Path::new(path)) else {
            return false;
        };
        let wd = workdir
            .filter(|w| !w.trim().is_empty())
            .and_then(|w| PathScope::tree(w, &self.env).ok());
        let mut queue = VecDeque::from(first);
        let mut seen = 0usize;
        while let Some(entry) = queue.pop_front() {
            seen += 1;
            if seen > MAX_ENTRIES {
                return true;
            }
            let text = entry.path.to_string_lossy();
            if PathScope::exact(&text, &self.env).is_ok_and(|s| is_protected_in(&s, wd.as_ref())) {
                return true;
            }
            if entry.is_dir {
                match self.fs.list_dir(&entry.path) {
                    Ok(more) => queue.extend(more),
                    Err(_) => return true,
                }
            }
        }
        false
    }
}
