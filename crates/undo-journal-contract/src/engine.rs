//! Rdzeń dziennika: kroki i operacje `fs.*` wykonywane przez dziennik (brak wpisu = brak
//! operacji). Kolejność: pre-image → operacja platformy → wpis; błąd wpisu cofa operację.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use platform_contract::{FsPort, OpReceipt, PlatformError};

use crate::Clock;
use crate::store::{JournalStore, sha256_hex, state_of};
use crate::types::{
    BlobId, FileState, JournalEntry, JournalRecord, StepCtx, StepId, UndoError, UndoLimits, UndoOp,
};

/// Krok w pamięci.
pub(crate) struct StepRec {
    pub ctx: StepCtx,
    pub started_ms: u64,
    pub entries: Vec<JournalEntry>,
    pub posts: BTreeMap<u32, BTreeMap<PathBuf, String>>,
    pub committed_ms: Option<u64>,
    pub undone: bool,
    pub expired: bool,
}

pub(crate) struct JState {
    pub steps: BTreeMap<StepId, StepRec>,
    pub next_step: u64,
}

/// Dziennik cofania nad `FsPort` i magazynem.
pub struct Journal {
    pub(crate) fs: Arc<dyn FsPort>,
    pub(crate) store: Arc<dyn JournalStore>,
    pub(crate) limits: UndoLimits,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) boot: u64,
    pub(crate) prefer_platform_undo: bool,
    pub(crate) state: Mutex<JState>,
}

impl Journal {
    /// Otwiera dziennik i odtwarza kroki z magazynu. `boot` identyfikuje uruchomienie —
    /// tokeny platformy z innego uruchomienia są pomijane (przywracanie z pre-image).
    pub fn open(
        fs: Arc<dyn FsPort>,
        store: Arc<dyn JournalStore>,
        limits: UndoLimits,
        clock: Arc<dyn Clock>,
        boot: u64,
    ) -> Result<Self, UndoError> {
        let mut steps: BTreeMap<StepId, StepRec> = BTreeMap::new();
        let mut next_step = 0;
        for rec in store.load().map_err(UndoError::Store)? {
            match rec {
                JournalRecord::Begin { step, ctx, at_ms } => {
                    next_step = next_step.max(step.0);
                    steps.insert(
                        step,
                        StepRec {
                            ctx,
                            started_ms: at_ms,
                            entries: Vec::new(),
                            posts: BTreeMap::new(),
                            committed_ms: None,
                            undone: false,
                            expired: false,
                        },
                    );
                }
                JournalRecord::Op(e) => {
                    if let Some(s) = steps.get_mut(&e.step) {
                        s.entries.push(e);
                    }
                }
                JournalRecord::Commit { step, at_ms, posts } => {
                    if let Some(s) = steps.get_mut(&step) {
                        s.committed_ms = Some(at_ms);
                        s.posts = posts;
                    }
                }
                JournalRecord::Undone { step, .. } => {
                    if let Some(s) = steps.get_mut(&step) {
                        s.undone = true;
                    }
                }
                JournalRecord::Expired { step } => {
                    if let Some(s) = steps.get_mut(&step) {
                        s.expired = true;
                    }
                }
            }
        }
        Ok(Self {
            fs,
            store,
            limits,
            clock,
            boot,
            prefer_platform_undo: true,
            state: Mutex::new(JState { steps, next_step }),
        })
    }

    /// Wyłącza tokeny platformy (test ścieżki zapasowej z pre-image).
    #[must_use]
    pub fn without_platform_undo(mut self) -> Self {
        self.prefer_platform_undo = false;
        self
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, JState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Stan pliku teraz (odczyt przez platformę).
    pub(crate) fn current(&self, path: &Path) -> Result<(FileState, Option<Vec<u8>>), UndoError> {
        match self.fs.read(path) {
            Ok(data) => Ok((state_of(&data), Some(data))),
            Err(PlatformError::NotFound(_)) => Ok((FileState::Absent, None)),
            Err(e) => Err(UndoError::Platform(e)),
        }
    }

    /// Rozpoczyna krok.
    pub fn begin_step(&self, ctx: StepCtx) -> Result<StepId, UndoError> {
        let now = self.clock.now_ms();
        let mut st = self.lock();
        let step = StepId(st.next_step + 1);
        let rec = JournalRecord::Begin {
            step,
            ctx: ctx.clone(),
            at_ms: now,
        };
        self.store.append(&rec).map_err(UndoError::Store)?;
        st.next_step = step.0;
        st.steps.insert(
            step,
            StepRec {
                ctx,
                started_ms: now,
                entries: Vec::new(),
                posts: BTreeMap::new(),
                committed_ms: None,
                undone: false,
                expired: false,
            },
        );
        Ok(step)
    }

    pub(crate) fn open_ctx(&self, step: StepId) -> Result<StepCtx, UndoError> {
        let st = self.lock();
        let s = st.steps.get(&step).ok_or(UndoError::UnknownStep(step))?;
        if s.committed_ms.is_some() || s.undone {
            return Err(UndoError::BadState {
                step,
                reason: "krok jest już zamknięty".into(),
            });
        }
        Ok(s.ctx.clone())
    }

    /// Zachowuje pre-image (limit pliku i magazynu); `None` = bez pre-image (za duży, a krok ma
    /// zgodę na nieodwracalność).
    pub(crate) fn keep_pre_image(
        &self,
        ctx: &StepCtx,
        path: &Path,
        data: &[u8],
    ) -> Result<Option<BlobId>, UndoError> {
        let size = data.len() as u64;
        if size > self.limits.pre_image_max_bytes {
            if ctx.allow_irreversible {
                return Ok(None);
            }
            return Err(UndoError::PreImageTooLarge {
                path: path.to_path_buf(),
                size,
                limit: self.limits.pre_image_max_bytes,
            });
        }
        if self.store.blob_bytes().saturating_add(size) > self.limits.store_limit_bytes {
            self.make_room(size);
        }
        if self.store.blob_bytes().saturating_add(size) > self.limits.store_limit_bytes {
            return if ctx.allow_irreversible {
                Ok(None)
            } else {
                Err(UndoError::StoreFull)
            };
        }
        self.store
            .put_blob(data)
            .map(Some)
            .map_err(UndoError::Store)
    }

    /// Dopisuje wpis; błąd zapisu cofa operację platformy (brak wpisu = brak operacji).
    pub(crate) fn record(
        &self,
        step: StepId,
        op: UndoOp,
        receipt: &OpReceipt,
    ) -> Result<(), UndoError> {
        let mut st = self.lock();
        let seq = st
            .steps
            .get(&step)
            .map_or(0, |s| u32::try_from(s.entries.len()).unwrap_or(u32::MAX));
        let entry = JournalEntry {
            step,
            seq,
            op,
            platform_undo: receipt.undo,
            boot: self.boot,
        };
        if let Err(e) = self.store.append(&JournalRecord::Op(entry.clone())) {
            if let Some(token) = receipt.undo {
                let _ = self.fs.undo(token);
            }
            return Err(UndoError::Store(e));
        }
        if let Some(s) = st.steps.get_mut(&step) {
            s.entries.push(entry);
        }
        Ok(())
    }

    /// Zapis pliku przez dziennik.
    pub fn write(&self, step: StepId, path: &Path, data: &[u8]) -> Result<(), UndoError> {
        let ctx = self.open_ctx(step)?;
        let (before, old) = self.current(path)?;
        let pre_image = match &old {
            Some(bytes) => self.keep_pre_image(&ctx, path, bytes)?,
            None => None,
        };
        let receipt = self
            .fs
            .write_atomic(path, data)
            .map_err(UndoError::Platform)?;
        let op = UndoOp::Write {
            path: path.to_path_buf(),
            before,
            pre_image,
            after: state_of(data),
        };
        self.record(step, op, &receipt)
    }

    /// Kopia przez dziennik.
    pub fn copy(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError> {
        self.open_ctx(step)?;
        let (_, data) = self.current(from)?;
        let receipt = self.fs.copy(from, to).map_err(UndoError::Platform)?;
        let after = data.as_deref().map_or(FileState::Absent, state_of);
        let op = UndoOp::Copy {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            after,
        };
        self.record(step, op, &receipt)
    }

    /// Przeniesienie przez dziennik.
    pub fn move_path(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError> {
        self.open_ctx(step)?;
        let (_, data) = self.current(from)?;
        let receipt = self.fs.move_path(from, to).map_err(UndoError::Platform)?;
        let after = data.as_deref().map_or(FileState::Absent, state_of);
        let op = UndoOp::Move {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            after,
        };
        self.record(step, op, &receipt)
    }

    /// Usunięcie do Kosza przez dziennik (pre-image jako zapas po restarcie, jeśli się mieści).
    pub fn delete(&self, step: StepId, path: &Path) -> Result<(), UndoError> {
        let ctx = self.open_ctx(step)?;
        let (before, old) = self.current(path)?;
        let fallback = StepCtx {
            allow_irreversible: true,
            ..ctx
        };
        let pre_image = match &old {
            Some(bytes) => self.keep_pre_image(&fallback, path, bytes)?,
            None => None,
        };
        let receipt = self
            .fs
            .delete_to_recycle_bin(path)
            .map_err(UndoError::Platform)?;
        let op = UndoOp::Delete {
            path: path.to_path_buf(),
            before,
            pre_image,
        };
        self.record(step, op, &receipt)
    }

    /// Trwałe usunięcie — odwracalne tylko dzięki pre-image (bez niego wymaga zgody kroku).
    pub fn delete_permanent(&self, step: StepId, path: &Path) -> Result<(), UndoError> {
        let ctx = self.open_ctx(step)?;
        let (before, old) = self.current(path)?;
        let pre_image = match &old {
            Some(bytes) => self.keep_pre_image(&ctx, path, bytes)?,
            None => None,
        };
        let receipt = self
            .fs
            .delete_permanent(path)
            .map_err(UndoError::Platform)?;
        let op = UndoOp::DeletePermanent {
            path: path.to_path_buf(),
            before,
            pre_image,
        };
        self.record(step, op, &receipt)
    }

    /// Hash treści (dla testów i UI).
    pub fn hash(data: &[u8]) -> String {
        sha256_hex(data)
    }
}
