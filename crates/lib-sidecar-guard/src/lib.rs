//! Rejestr procesów pobocznych Alfy (`llama-server`, `whisper-server`, silniki głosu, Smart Turn).
//!
//! Moduł uruchamiający proces woła [`register`], po jego zakończeniu [`unregister`]. Powłoka woła
//! [`kill_all`] przy wyjściu z programu (wyjście przez `exit` nie niszczy obiektów procesów, więc
//! `kill_on_drop` nie działa) oraz [`init`] przy starcie — rejestr poprzedniego uruchomienia leży
//! na dysku i pozwala zakończyć procesy osierocone przez awarię.
//!
//! Numery procesów są używane ponownie, dlatego proces jest kończony tylko wtedy, gdy zgadza się
//! jego tożsamość (audyt 2026-10-14, A-13): nazwa pliku, a gdy udało się ją zapisać przy starcie
//! procesu — także pełna ścieżka pliku i czas utworzenia procesu. Odcisk jest zbierany zaraz po
//! uruchomieniu (w tle) jednym zapytaniem do systemu; sprawdzenie przed zakończeniem — jednym
//! zapytaniem dla wszystkich procesów naraz, tuż przed `taskkill`.

mod os;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

pub use os::Seen;

/// Wpis rejestru: nazwa pliku obrazu i (gdy znany) odcisk procesu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Nazwa pliku obrazu (małymi literami).
    pub image: String,
    /// Odcisk z chwili uruchomienia: pełna ścieżka (małymi literami) i czas utworzenia.
    pub seen: Option<Seen>,
}

impl Entry {
    /// Czy proces zastany pod tym numerem to ten sam proces, który zgłoszono.
    pub fn matches(&self, now: &Seen) -> bool {
        now.image == self.image && self.seen.as_ref().is_none_or(|s| s == now)
    }
}

#[derive(Default)]
struct State {
    /// Plik rejestru (`None` — bez zapisu na dysk, np. w testach modułów).
    file: Option<PathBuf>,
    /// Numer procesu → wpis.
    children: BTreeMap<u32, Entry>,
}

fn state() -> MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

fn image_name(program: &Path) -> String {
    program
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Wiersz rejestru: `numer<TAB>nazwa[<TAB>ścieżka<TAB>czas]`.
fn line(pid: u32, e: &Entry) -> String {
    match &e.seen {
        Some(s) => format!("{pid}\t{}\t{}\t{}\n", e.image, s.path, s.created),
        None => format!("{pid}\t{}\n", e.image),
    }
}

fn persist(state: &State) {
    let Some(file) = &state.file else { return };
    let text: String = state
        .children
        .iter()
        .map(|(pid, e)| line(*pid, e))
        .collect();
    if let Err(error) = std::fs::write(file, text) {
        tracing::warn!(plik = %file.display(), %error, "rejestr procesów pobocznych: zapis nieudany");
    }
}

/// Wpisy rejestru z pliku; błędne wiersze są pomijane, niepełny odcisk — traktowany jak brak.
fn parse(text: &str) -> Vec<(u32, Entry)> {
    text.lines()
        .filter_map(|row| {
            let mut cols = row.split('\t');
            let pid = cols.next()?.trim().parse().ok()?;
            let image = cols.next()?.trim().to_lowercase();
            if image.is_empty() {
                return None;
            }
            let seen = match (cols.next(), cols.next()) {
                (Some(path), Some(created)) if !path.trim().is_empty() => {
                    created.trim().parse().ok().map(|created| Seen {
                        image: image.clone(),
                        path: path.trim().to_lowercase(),
                        created,
                    })
                }
                _ => None,
            };
            Some((pid, Entry { image, seen }))
        })
        .collect()
}

/// Kończy procesy, których tożsamość się zgadza. Bez odpowiedzi systemu (zapytanie nieudane)
/// — tylko po nazwie pliku, jak dawniej.
fn kill_matching(entries: &[(u32, Entry)]) -> usize {
    let pids: Vec<u32> = entries.iter().map(|(pid, _)| *pid).collect();
    let now = os::snapshot(&pids);
    let mut killed = 0;
    for (pid, entry) in entries {
        let same = match &now {
            Some(seen) => seen.get(pid).is_some_and(|s| entry.matches(s)),
            None => os::alive_as(*pid, &entry.image),
        };
        if same && os::kill(*pid) {
            killed += 1;
        } else if !same && now.as_ref().is_some_and(|s| s.contains_key(pid)) {
            tracing::info!(pid, obraz = %entry.image, "numer procesu zajęty przez inny proces — nietknięty");
        }
    }
    killed
}

/// Ustawia plik rejestru i kończy procesy osierocone przez poprzednie uruchomienie. Zwraca liczbę
/// zakończonych procesów.
pub fn init(file: PathBuf) -> usize {
    let previous = std::fs::read_to_string(&file)
        .map(|text| parse(&text))
        .unwrap_or_default();
    let killed = kill_matching(&previous);
    if killed > 0 {
        tracing::warn!(
            killed,
            "zakończono procesy poboczne osierocone przez poprzednie uruchomienie"
        );
    }
    let mut st = state();
    st.file = Some(file);
    persist(&st);
    killed
}

/// Zgłasza uruchomiony proces poboczny; odcisk (ścieżka, czas utworzenia) dopisywany w tle.
pub fn register(pid: u32, program: &Path) {
    let image = image_name(program);
    if pid == 0 || image.is_empty() {
        return;
    }
    {
        let mut st = state();
        st.children.insert(
            pid,
            Entry {
                image: image.clone(),
                seen: None,
            },
        );
        persist(&st);
    }
    // Zapytanie do systemu trwa ułamek sekundy — nie blokuje modułu, który uruchomił proces.
    let spawned = std::thread::Builder::new()
        .name("sidecar-odcisk".into())
        .spawn(move || fingerprint(pid, &image));
    if let Err(error) = spawned {
        tracing::debug!(%error, "odcisk procesu pobocznego: wątek nieuruchomiony");
    }
}

/// Zapisuje odcisk procesu, jeśli pod numerem działa nadal zgłoszony plik.
fn fingerprint(pid: u32, image: &str) {
    let Some(seen) = os::snapshot(&[pid]).and_then(|mut s| s.remove(&pid)) else {
        return;
    };
    if seen.image != image {
        return;
    }
    let mut st = state();
    if let Some(entry) = st.children.get_mut(&pid)
        && entry.image == image
    {
        entry.seen = Some(seen);
        persist(&st);
    }
}

/// Wykreśla proces, który się zakończył albo został zabity przez swój moduł.
pub fn unregister(pid: u32) {
    let mut st = state();
    if st.children.remove(&pid).is_some() {
        persist(&st);
    }
}

/// Kończy wszystkie zgłoszone procesy, które jeszcze działają (z ich procesami potomnymi).
/// Zwraca liczbę zakończonych.
pub fn kill_all() -> usize {
    let children: Vec<(u32, Entry)> = std::mem::take(&mut state().children).into_iter().collect();
    let killed = kill_matching(&children);
    persist(&state());
    killed
}

/// Liczba zgłoszonych procesów (diagnostyka, testy).
pub fn registered() -> usize {
    state().children.len()
}

/// Odcisk zgłoszonego procesu (testy, diagnostyka).
pub fn fingerprint_of(pid: u32) -> Option<Seen> {
    state().children.get(&pid).and_then(|e| e.seen.clone())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
