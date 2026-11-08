//! Zawartość katalogu przed jego usunięciem, przeniesieniem albo kopią (plan 50 §8, luka 3
//! z przeglądu 3.11). Sędzia ocenia ścieżki leksykalnie, więc usunięcie katalogu z podrepozytorium (`.git`)
//! albo z `.vscode` nie było „zmianą ścieżki chronionej”. Narzędzie przegląda drzewo na dysku
//! i zgłasza Sędziemu fakt `contains_protected`; przy zbyt dużym albo nieczytelnym drzewie — też
//! (ostrożnie: zapyta).

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

use compliance_contract::PathEnv;
use platform_contract::FsPort;
use safety_broker_contract::{PathScope, is_protected_in};

use crate::core::Core;

/// Najwięcej wpisów przeglądanych przed uznaniem drzewa za „może zawierać”.
const MAX_ENTRIES: usize = 5_000;

impl Core {
    /// Czy katalog `path` zawiera ścieżkę chronioną (plik albo brak — `false`). Przegląd dysku
    /// (do 5000 wpisów) w wątku blokującym tokio, żeby nie wstrzymywać wątków asynchronicznych
    /// (plan 50 §8, decyzja właściciela 6.11); poza środowiskiem tokio — w miejscu.
    pub(crate) async fn holds_protected(&self, path: &str, workdir: Option<&str>) -> bool {
        let (fs, env) = (Arc::clone(&self.fs), self.env.clone());
        let (path, workdir) = (path.to_owned(), workdir.map(str::to_owned));
        let scan = move || scan(fs.as_ref(), &env, &path, workdir.as_deref());
        match tokio::runtime::Handle::try_current() {
            // Przerwany wątek przeglądu — ostrożnie: „może zawierać”, Sędzia zapyta.
            Ok(handle) => handle.spawn_blocking(scan).await.unwrap_or(true),
            Err(_) => scan(),
        }
    }
}

/// Przegląd drzewa wszerz; zbyt duże albo nieczytelne drzewo — `true` (ostrożnie).
fn scan(fs: &dyn FsPort, env: &PathEnv, path: &str, workdir: Option<&str>) -> bool {
    let Ok(first) = fs.list_dir(Path::new(path)) else {
        return false;
    };
    let wd = workdir
        .filter(|w| !w.trim().is_empty())
        .and_then(|w| PathScope::tree(w, env).ok());
    let mut queue = VecDeque::from(first);
    let mut seen = 0usize;
    while let Some(entry) = queue.pop_front() {
        seen += 1;
        if seen > MAX_ENTRIES {
            return true;
        }
        let text = entry.path.to_string_lossy();
        if PathScope::exact(&text, env).is_ok_and(|s| is_protected_in(&s, wd.as_ref())) {
            return true;
        }
        if entry.is_dir {
            match fs.list_dir(&entry.path) {
                Ok(more) => queue.extend(more),
                Err(_) => return true,
            }
        }
    }
    false
}
