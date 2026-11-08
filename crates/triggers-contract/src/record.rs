//! Wejścia zdarzeniowe, przyczyny wyzwolenia, dziennik uruchomień, widok wyzwalacza.

use core_bus_contract::SessionId;
use safety_broker_contract::TaintSource;
use scheduler_contract::{TaskId, TaskOrigin};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::{Actor, TriggerId, TriggerSpec};

/// Wejście zdarzeniowe (dostarcza powłoka: obserwator plików, magistrala).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum TriggerInput {
    /// Pojawił się plik.
    FileCreated {
        /// Pełna ścieżka.
        path: String,
    },
    /// Nowa wiadomość w sesji.
    NewMessage {
        /// Sesja.
        session: SessionId,
        /// Tura.
        turn: String,
        /// Rola autora (`user`, `assistant`, `tool`…).
        role: String,
    },
    /// Zakończyło się zadanie schedulera.
    TaskFinished {
        /// Zadanie.
        task: TaskId,
        /// Wynik (`succeeded`, `failed`, `cancelled`, …).
        result: String,
        /// Pochodzenie zadania (ochrona przed pętlą wyzwalaczy).
        origin: TaskOrigin,
        /// Taint zadania (przenoszony na wyzwolone zadanie).
        #[serde(default)]
        taint: Vec<TaintSource>,
    },
}

/// Przyczyna wyzwolenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "cause", rename_all = "snake_case")]
pub enum FireCause {
    /// Termin czasowy (`scheduled_ms` = planowana chwila; po zaległości — pierwsza przegapiona).
    Time {
        /// Planowana chwila.
        scheduled_ms: u64,
        /// Ile wystąpień przegapiono (0 = punktualnie).
        missed: u32,
    },
    /// Ręcznie.
    Manual {
        /// Kto.
        by: Actor,
    },
    /// Plik (treść niezaufana).
    File {
        /// Ścieżka.
        path: String,
    },
    /// Wiadomość (treść niezaufana).
    Message {
        /// Sesja.
        session: SessionId,
        /// Tura.
        turn: String,
    },
    /// Koniec zadania.
    TaskFinished {
        /// Zadanie.
        task: TaskId,
        /// Wynik.
        result: String,
        /// Głębokość łańcucha wyzwalaczy zadania.
        depth: u32,
        /// Taint zakończonego zadania (przenoszony dalej).
        #[serde(default)]
        taint: Vec<TaintSource>,
    },
    /// Uruchomienie odłożone z okna ciszy.
    Deferred {
        /// Pierwotna przyczyna.
        original: Box<FireCause>,
    },
}

/// Dlaczego wyzwolenie nie dało zadania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SuppressReason {
    /// Limit częstości wyzwalacza.
    RateLimited,
    /// Globalny limit wszystkich wyzwalaczy.
    GlobalRateLimited,
    /// Okno ciszy (tryb „pomiń”).
    Quiet,
    /// „Nie przeszkadzać” (tryb „pomiń”).
    Dnd,
    /// Za długi łańcuch wyzwalaczy (zadanie z wyzwalacza wyzwala kolejny…).
    ChainTooDeep,
    /// Wyzwalacz zareagowałby na własne zadanie.
    SelfLoop,
    /// Przegapione (polityka `skip`).
    Missed,
}

/// Wynik wyzwolenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RunOutcome {
    /// Zadanie zgłoszone do schedulera.
    Submitted {
        /// Zadanie.
        task: TaskId,
    },
    /// Pominięte.
    Suppressed {
        /// Powód.
        reason: SuppressReason,
    },
    /// Odłożone do końca ciszy.
    Deferred {
        /// Do kiedy (ms).
        until_ms: u64,
    },
    /// Scheduler odrzucił zadanie.
    Failed {
        /// Błąd.
        error: String,
    },
}

/// Wpis dziennika uruchomień.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunRecord {
    /// Chwila (ms).
    pub at_ms: u64,
    /// Wyzwalacz.
    pub trigger: TriggerId,
    /// Przyczyna.
    pub cause: FireCause,
    /// Wynik.
    pub outcome: RunOutcome,
}

/// Widok wyzwalacza (UI, testy).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TriggerView {
    /// Specyfikacja.
    pub spec: TriggerSpec,
    /// Następne wystąpienie czasowe (ms).
    pub next_fire_ms: Option<u64>,
    /// Ostatnie wyzwolenie (ms).
    pub last_fire_ms: Option<u64>,
    /// Liczba zgłoszonych zadań.
    pub fired: u64,
    /// Liczba pominiętych.
    pub suppressed: u64,
    /// Odłożone uruchomienie (do kiedy).
    pub deferred_until_ms: Option<u64>,
}

/// Prosty wzorzec nazwy pliku: `*` (dowolny ciąg), `?` (jeden znak), bez rozróżniania
/// wielkości liter (Windows).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Ścieżka znormalizowana do porównań (`/`, małe litery, bez końcowego separatora).
pub fn norm_path(path: &str) -> String {
    let mut p = path.trim().replace('\\', "/").to_lowercase();
    while p.len() > 1 && p.ends_with('/') {
        p.pop();
    }
    p
}

/// Czy plik leży bezpośrednio w katalogu i pasuje do wzorca.
pub fn file_matches(dir: &str, pattern: Option<&str>, path: &str) -> bool {
    let path = norm_path(path);
    let Some((parent, name)) = path.rsplit_once('/') else {
        return false;
    };
    parent == norm_path(dir) && pattern.is_none_or(|p| glob_match(p, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_and_paths() {
        assert!(glob_match("*.pdf", "Faktura.PDF"));
        assert!(glob_match("f?ktura*", "faktura-2026.pdf"));
        assert!(!glob_match("*.pdf", "x.pdfx"));
        assert!(glob_match("*", ""));
        assert!(glob_match("a*b*c", "aXXbYYc"));
        assert!(!glob_match("a*b*c", "aXXbYY"));
        assert!(file_matches(
            "C:\\Users\\Ja\\Downloads\\",
            Some("*.pdf"),
            "c:/users/ja/downloads/a.pdf"
        ));
        assert!(!file_matches(
            "C:\\Users\\Ja\\Downloads",
            None,
            "C:\\Users\\Ja\\Downloads\\sub\\a.pdf"
        ));
        assert!(!file_matches("C:\\x", None, "plik"));
        assert_eq!(norm_path("C:\\A\\"), "c:/a");
    }
}
