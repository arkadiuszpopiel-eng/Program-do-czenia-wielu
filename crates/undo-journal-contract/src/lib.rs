//! Kontrakt dziennika cofania (docs/modules/undo-journal/SPEC.md, PLAN §8.7, §14.8).
//!
//! Każda operacja `fs.*` agentki przechodzi przez dziennik ([`Journal`]): pre-image (z limitem
//! rozmiaru i magazynu, retencją), operacja przez `FsPort` (token cofnięcia platformy), wpis
//! append-only; brak wpisu = brak operacji. Kroki grupują operacje („Delta: przeniesiono 14
//! plików · Cofnij”). Cofnięcie najpierw sprawdza konflikty całego kroku (plik zmieniony
//! później → czytelny błąd, nic nie jest ruszane), potem odtwarza stan w odwrotnej kolejności.
//! Shell w zakresie: snapshot zakresu (kopia plików z limitem). Rdzeń jest tutaj (jak
//! `scheduler-lite`), żeby `-impl` (magazyn katalogowy) i `-fake` (pamięć) nie mogły się
//! rozjechać.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod engine;
mod lifecycle;
mod restore;
mod snapshot;
mod store;
mod types;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use engine::Journal;
pub use lifecycle::files_pl;
pub use store::{JournalStore, MemStore, sha256_hex, state_of};
pub use types::{
    BlobId, FileState, JournalEntry, JournalRecord, Manifest, OpCounts, StepCtx, StepId,
    StepSummary, UndoError, UndoLimits, UndoOp, UndoReport,
};

use std::path::Path;

use core_bus_contract::{EventKind, SessionId};

/// Zdarzenie (Audyt): zapisano krok.
pub const EVENT_RECORDED: &str = "undo.recorded";
/// Zdarzenie (Audyt): cofnięto krok (`ok`, `partial`).
pub const EVENT_UNDONE: &str = "undo.undone";
/// Zdarzenie: snapshot zakresu.
pub const EVENT_SNAPSHOT_CREATED: &str = "undo.snapshot.created";
/// Zdarzenie: retencja usunęła pre-image.
pub const EVENT_PRUNED: &str = "undo.pruned";
/// Zdarzenie (Audyt): cofnięcie nieudane (konflikt, częściowe).
pub const EVENT_FAILED: &str = "undo.failed";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Źródło czasu (ms).
pub trait Clock: Send + Sync {
    /// Bieżący czas.
    fn now_ms(&self) -> u64;
}

impl<F: Fn() -> u64 + Send + Sync> Clock for F {
    fn now_ms(&self) -> u64 {
        self()
    }
}

/// Dziennik cofania — kontrakt dla narzędzi `fs.*`/shell i UI („Cofnij”).
pub trait UndoJournal: Send + Sync {
    /// Rozpoczyna krok.
    fn begin_step(&self, ctx: StepCtx) -> Result<StepId, UndoError>;
    /// Zapis pliku.
    fn write(&self, step: StepId, path: &Path, data: &[u8]) -> Result<(), UndoError>;
    /// Kopia.
    fn copy(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError>;
    /// Przeniesienie.
    fn move_path(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError>;
    /// Usunięcie do Kosza.
    fn delete(&self, step: StepId, path: &Path) -> Result<(), UndoError>;
    /// Trwałe usunięcie (odwracalne tylko przez pre-image).
    fn delete_permanent(&self, step: StepId, path: &Path) -> Result<(), UndoError>;
    /// Snapshot zakresu przed poleceniem powłoki.
    fn snapshot_scope(&self, step: StepId, root: &Path) -> Result<(), UndoError>;
    /// Zatwierdza krok.
    fn commit_step(&self, step: StepId) -> Result<StepSummary, UndoError>;
    /// Przerywa niezatwierdzony krok (cofa wykonane operacje).
    fn abort_step(&self, step: StepId) -> Result<UndoReport, UndoError>;
    /// Cofa krok.
    fn undo(&self, step: StepId) -> Result<UndoReport, UndoError>;
    /// Cofa `n` ostatnich kroków sesji.
    fn undo_last(&self, session: &SessionId, n: usize) -> Result<Vec<UndoReport>, UndoError>;
    /// Kroki sesji.
    fn steps(&self, session: &SessionId) -> Vec<StepSummary>;
    /// Retencja.
    fn prune(&self) -> usize;
}

impl UndoJournal for Journal {
    fn begin_step(&self, ctx: StepCtx) -> Result<StepId, UndoError> {
        Journal::begin_step(self, ctx)
    }
    fn write(&self, step: StepId, path: &Path, data: &[u8]) -> Result<(), UndoError> {
        Journal::write(self, step, path, data)
    }
    fn copy(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError> {
        Journal::copy(self, step, from, to)
    }
    fn move_path(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError> {
        Journal::move_path(self, step, from, to)
    }
    fn delete(&self, step: StepId, path: &Path) -> Result<(), UndoError> {
        Journal::delete(self, step, path)
    }
    fn delete_permanent(&self, step: StepId, path: &Path) -> Result<(), UndoError> {
        Journal::delete_permanent(self, step, path)
    }
    fn snapshot_scope(&self, step: StepId, root: &Path) -> Result<(), UndoError> {
        Journal::snapshot_scope(self, step, root)
    }
    fn commit_step(&self, step: StepId) -> Result<StepSummary, UndoError> {
        Journal::commit_step(self, step)
    }
    fn abort_step(&self, step: StepId) -> Result<UndoReport, UndoError> {
        Journal::abort_step(self, step)
    }
    fn undo(&self, step: StepId) -> Result<UndoReport, UndoError> {
        Journal::undo(self, step)
    }
    fn undo_last(&self, session: &SessionId, n: usize) -> Result<Vec<UndoReport>, UndoError> {
        Journal::undo_last(self, session, n)
    }
    fn steps(&self, session: &SessionId) -> Vec<StepSummary> {
        Journal::steps(self, session)
    }
    fn prune(&self) -> usize {
        Journal::prune(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_plurals() {
        assert_eq!(files_pl(1), "1 plik");
        assert_eq!(files_pl(3), "3 pliki");
        assert_eq!(files_pl(5), "5 plików");
        assert_eq!(files_pl(12), "12 plików");
        assert_eq!(files_pl(14), "14 plików");
        assert_eq!(files_pl(22), "22 pliki");
        assert_eq!(files_pl(0), "0 plików");
    }
}
