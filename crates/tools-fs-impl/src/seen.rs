//! Pliki znane przebiegowi: odczytane przez `fs_read` albo zapisane przez `fs_write`. Nadpisanie
//! (`overwrite`) istniejącego, niepustego pliku, którego przebieg nie zna, nie jest wykonywane —
//! lekki model zastępował treść umowy zdaniem „Zawartość umowy z zamienionymi nazwiskami…” bez
//! odczytu, a weryfikacja tym samym modelem uznawała wynik za poprawny (pomiar F3 fs-11, fs-13, 2.11).
//! Poza przebiegiem (`ToolCtx::run` puste — wywołanie z interfejsu) blokady nie ma.

use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;

use core_bus_contract::RunId;
use tools_common_contract::{ToolCtx, ToolErrorKind, ToolOutcome};

/// Ilu ostatnich przebiegów pliki są pamiętane.
const RUNS: usize = 32;
/// Najwięcej plików pamiętanych dla jednego przebiegu (dalsze nie są dopisywane).
const PATHS: usize = 1024;

/// Pliki znane ostatnim przebiegom (najstarszy przebieg wypada pierwszy).
#[derive(Default)]
pub(crate) struct Seen {
    runs: Mutex<VecDeque<(RunId, HashSet<String>)>>,
}

impl Seen {
    /// Przebieg zna już treść pliku `path` (odczytał go albo zapisał).
    pub(crate) fn note(&self, ctx: &ToolCtx, path: &str) {
        let (Some(run), Ok(mut runs)) = (&ctx.run, self.runs.lock()) else {
            return;
        };
        if let Some((_, paths)) = runs.iter_mut().find(|(r, _)| r == run) {
            if paths.len() < PATHS {
                paths.insert(key(path));
            }
            return;
        }
        if runs.len() >= RUNS {
            runs.pop_front();
        }
        runs.push_back((run.clone(), HashSet::from([key(path)])));
    }

    /// Czy przebieg zna treść pliku (poza przebiegiem — zawsze tak).
    pub(crate) fn knows(&self, ctx: &ToolCtx, path: &str) -> bool {
        let Some(run) = &ctx.run else {
            return true;
        };
        let Ok(runs) = self.runs.lock() else {
            return true;
        };
        runs.iter()
            .any(|(r, paths)| r == run && paths.contains(&key(path)))
    }
}

/// Windows nie rozróżnia wielkości liter ani ukośników w ścieżkach.
fn key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

/// Wynik dla nadpisania pliku, którego przebieg nie odczytał.
pub(crate) fn unread(action: &str) -> ToolOutcome {
    ToolOutcome::failed(
        ToolErrorKind::InvalidArgs,
        format!(
            "Nie wykonano: {action} — plik istnieje i ma treść, a w tym zadaniu go nie odczytałaś; \
             nadpisanie zastąpiłoby całą treść. Najpierw odczytaj plik (fs_read), potem zapisz trybem \
             `overwrite` całą nową treść: zmienione miejsca i resztę bez zmian."
        ),
    )
}
