//! Atrapa dziennika cofania (docs/modules/undo-journal/SPEC.md, sekcja „Fake”): rdzeń z
//! kontraktu, magazyn w pamięci, `FsPort` opakowany w [`FlakyFs`] ze sterowanymi błędami
//! przywracania (chaos: przerwanie w trakcie cofania → raport częściowy, pre-image nietknięte).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use core_bus_contract::SessionId;
use platform_contract::{DirEntry, FsPort, KnownFolder, OpReceipt, PlatformError, UndoToken};
use undo_journal_contract::{
    Clock, Journal, MemStore, StepCtx, StepId, StepSummary, UndoError, UndoJournal, UndoLimits,
    UndoReport,
};

/// `FsPort` z wstrzykiwanymi awariami operacji mutujących (po `arm`).
pub struct FlakyFs {
    inner: Arc<dyn FsPort>,
    armed: AtomicBool,
    fail_after: AtomicU32,
}

impl FlakyFs {
    /// Opakowuje port.
    pub fn new(inner: Arc<dyn FsPort>) -> Self {
        Self {
            inner,
            armed: AtomicBool::new(false),
            fail_after: AtomicU32::new(0),
        }
    }

    /// Po `ok` udanych operacjach mutujących każda kolejna kończy się błędem we/wy.
    pub fn arm(&self, ok: u32) {
        self.fail_after.store(ok, Ordering::SeqCst);
        self.armed.store(true, Ordering::SeqCst);
    }

    /// Wyłącza awarie.
    pub fn disarm(&self) {
        self.armed.store(false, Ordering::SeqCst);
    }

    fn gate(&self) -> Result<(), PlatformError> {
        if !self.armed.load(Ordering::SeqCst) {
            return Ok(());
        }
        let left = self.fail_after.load(Ordering::SeqCst);
        if left == 0 {
            return Err(PlatformError::Io("awaria wstrzyknięta (chaos)".into()));
        }
        self.fail_after.store(left - 1, Ordering::SeqCst);
        Ok(())
    }
}

impl FsPort for FlakyFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PlatformError> {
        self.inner.read(path)
    }
    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<OpReceipt, PlatformError> {
        self.gate()?;
        self.inner.write_atomic(path, data)
    }
    fn copy(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        self.gate()?;
        self.inner.copy(from, to)
    }
    fn move_path(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        self.gate()?;
        self.inner.move_path(from, to)
    }
    fn delete_to_recycle_bin(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        self.gate()?;
        self.inner.delete_to_recycle_bin(path)
    }
    fn delete_permanent(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        self.gate()?;
        self.inner.delete_permanent(path)
    }
    fn exists(&self, path: &Path) -> bool {
        self.inner.exists(path)
    }
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        self.inner.list_dir(path)
    }
    fn undo(&self, token: UndoToken) -> Result<(), PlatformError> {
        self.gate()?;
        self.inner.undo(token)
    }
    fn known_folder(&self, folder: KnownFolder) -> PathBuf {
        self.inner.known_folder(folder)
    }
}

/// Atrapa dziennika.
pub struct FakeUndoJournal {
    journal: Journal,
    flaky: Arc<FlakyFs>,
    store: Arc<MemStore>,
}

impl FakeUndoJournal {
    /// Dziennik w pamięci nad `fs` (z wirtualnym zegarem).
    pub fn new(
        fs: Arc<dyn FsPort>,
        limits: UndoLimits,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, UndoError> {
        let flaky = Arc::new(FlakyFs::new(fs));
        let store = Arc::new(MemStore::default());
        let journal = Journal::open(flaky.clone(), store.clone(), limits, clock, 1)?;
        Ok(Self {
            journal,
            flaky,
            store,
        })
    }

    /// Sterowanie awariami platformy (operacje i przywracanie).
    pub fn flaky(&self) -> &FlakyFs {
        &self.flaky
    }

    /// Magazyn (np. `set_fail_append`, liczba pre-image).
    pub fn store(&self) -> &MemStore {
        &self.store
    }
}

impl UndoJournal for FakeUndoJournal {
    fn begin_step(&self, ctx: StepCtx) -> Result<StepId, UndoError> {
        self.journal.begin_step(ctx)
    }
    fn write(&self, step: StepId, path: &Path, data: &[u8]) -> Result<(), UndoError> {
        self.journal.write(step, path, data)
    }
    fn copy(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError> {
        self.journal.copy(step, from, to)
    }
    fn move_path(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError> {
        self.journal.move_path(step, from, to)
    }
    fn delete(&self, step: StepId, path: &Path) -> Result<(), UndoError> {
        self.journal.delete(step, path)
    }
    fn delete_permanent(&self, step: StepId, path: &Path) -> Result<(), UndoError> {
        self.journal.delete_permanent(step, path)
    }
    fn snapshot_scope(&self, step: StepId, root: &Path) -> Result<(), UndoError> {
        self.journal.snapshot_scope(step, root)
    }
    fn commit_step(&self, step: StepId) -> Result<StepSummary, UndoError> {
        self.journal.commit_step(step)
    }
    fn abort_step(&self, step: StepId) -> Result<UndoReport, UndoError> {
        self.journal.abort_step(step)
    }
    fn undo(&self, step: StepId) -> Result<UndoReport, UndoError> {
        self.journal.undo(step)
    }
    fn undo_last(&self, session: &SessionId, n: usize) -> Result<Vec<UndoReport>, UndoError> {
        self.journal.undo_last(session, n)
    }
    fn steps(&self, session: &SessionId) -> Vec<StepSummary> {
        self.journal.steps(session)
    }
    fn prune(&self) -> usize {
        self.journal.prune()
    }
}
