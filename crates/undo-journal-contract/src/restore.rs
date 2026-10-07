//! Cofanie: kontrola konfliktów dla całego kroku przed jakąkolwiek zmianą, potem odtworzenie
//! w odwrotnej kolejności (token platformy z tego uruchomienia albo pre-image).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::engine::Journal;
use crate::types::{FileState, JournalEntry, JournalRecord, StepId, UndoError, UndoOp, UndoReport};

impl Journal {
    /// Oczekiwany stan końcowy ścieżek kroku (ostatnia operacja na ścieżce wygrywa).
    fn expected(entries: &[JournalEntry]) -> BTreeMap<PathBuf, FileState> {
        let mut out = BTreeMap::new();
        for e in entries {
            match &e.op {
                UndoOp::Write { path, after, .. } => {
                    out.insert(path.clone(), after.clone());
                }
                UndoOp::Copy { to, after, .. } => {
                    out.insert(to.clone(), after.clone());
                }
                UndoOp::Move { from, to, after } => {
                    out.insert(from.clone(), FileState::Absent);
                    out.insert(to.clone(), after.clone());
                }
                UndoOp::Delete { path, .. } | UndoOp::DeletePermanent { path, .. } => {
                    out.insert(path.clone(), FileState::Absent);
                }
                UndoOp::ScopeSnapshot { .. } => {}
            }
        }
        out
    }

    fn check_conflicts(
        &self,
        entries: &[JournalEntry],
        posts: &BTreeMap<u32, BTreeMap<PathBuf, String>>,
    ) -> Result<(), UndoError> {
        for (path, expected) in Self::expected(entries) {
            let (found, _) = self.current(&path)?;
            if found != expected {
                return Err(UndoError::Conflict {
                    path,
                    expected,
                    found,
                });
            }
        }
        for e in entries {
            if let UndoOp::ScopeSnapshot { root, .. } = &e.op {
                let post = posts.get(&e.seq).cloned().unwrap_or_default();
                let now = self.hashes(root)?;
                if now != post {
                    let path = now
                        .iter()
                        .find(|(p, h)| post.get(*p) != Some(*h))
                        .map(|(p, _)| p.clone())
                        .or_else(|| post.keys().find(|p| !now.contains_key(*p)).cloned())
                        .unwrap_or_else(|| root.clone());
                    let found = now
                        .get(&path)
                        .map_or(FileState::Absent, |h| FileState::Present {
                            hash: h.clone(),
                            len: 0,
                        });
                    let expected =
                        post.get(&path)
                            .map_or(FileState::Absent, |h| FileState::Present {
                                hash: h.clone(),
                                len: 0,
                            });
                    return Err(UndoError::Conflict {
                        path,
                        expected,
                        found,
                    });
                }
            }
        }
        Ok(())
    }

    fn platform_first(&self, e: &JournalEntry) -> bool {
        if !(self.prefer_platform_undo && e.boot == self.boot) {
            return false;
        }
        e.platform_undo.is_some_and(|t| self.fs.undo(t).is_ok())
    }

    fn blob(&self, id: Option<&crate::types::BlobId>) -> Result<Vec<u8>, String> {
        let id = id.ok_or("brak pre-image (operacja nieodwracalna)")?;
        self.store.get_blob(id)
    }

    /// Odtwarza jedną operację.
    fn restore(&self, e: &JournalEntry, failed: &mut Vec<(PathBuf, String)>) -> u32 {
        if let UndoOp::ScopeSnapshot { root, files } = &e.op {
            return self.restore_scope(root, files, failed);
        }
        if self.platform_first(e) {
            return 1;
        }
        let fs = &self.fs;
        let (path, res): (PathBuf, Result<(), String>) = match &e.op {
            UndoOp::Write {
                path,
                before,
                pre_image,
                ..
            } => (
                path.clone(),
                match before {
                    FileState::Absent => fs
                        .delete_to_recycle_bin(path)
                        .map(|_| ())
                        .map_err(|x| x.to_string()),
                    FileState::Present { .. } => self.blob(pre_image.as_ref()).and_then(|d| {
                        fs.write_atomic(path, &d)
                            .map(|_| ())
                            .map_err(|x| x.to_string())
                    }),
                },
            ),
            UndoOp::Copy { to, .. } => (
                to.clone(),
                fs.delete_to_recycle_bin(to)
                    .map(|_| ())
                    .map_err(|x| x.to_string()),
            ),
            UndoOp::Move { from, to, .. } => (
                from.clone(),
                fs.move_path(to, from)
                    .map(|_| ())
                    .map_err(|x| x.to_string()),
            ),
            UndoOp::Delete {
                path, pre_image, ..
            }
            | UndoOp::DeletePermanent {
                path, pre_image, ..
            } => (
                path.clone(),
                self.blob(pre_image.as_ref()).and_then(|d| {
                    fs.write_atomic(path, &d)
                        .map(|_| ())
                        .map_err(|x| x.to_string())
                }),
            ),
            UndoOp::ScopeSnapshot { root, .. } => (root.clone(), Ok(())),
        };
        match res {
            Ok(()) => 1,
            Err(why) => {
                failed.push((path, why));
                0
            }
        }
    }

    /// Cofa zatwierdzony krok. Konflikt → nic nie jest zmieniane; częściowe niepowodzenie →
    /// `UndoError::Partial` z raportem (krok oznaczony jako cofnięty, by nie powtarzać zmian).
    pub fn undo(&self, step: StepId) -> Result<UndoReport, UndoError> {
        let (entries, posts) = {
            let st = self.lock();
            let s = st.steps.get(&step).ok_or(UndoError::UnknownStep(step))?;
            if s.expired {
                return Err(UndoError::Expired(step));
            }
            if s.undone {
                return Err(UndoError::BadState {
                    step,
                    reason: "krok już cofnięty".into(),
                });
            }
            if s.committed_ms.is_none() {
                return Err(UndoError::BadState {
                    step,
                    reason: "krok niezatwierdzony (użyj abort)".into(),
                });
            }
            (s.entries.clone(), s.posts.clone())
        };
        self.check_conflicts(&entries, &posts)?;
        self.revert(step, &entries)
    }

    /// Przerywa niezatwierdzony krok, cofając wykonane operacje.
    pub fn abort_step(&self, step: StepId) -> Result<UndoReport, UndoError> {
        let entries = {
            let st = self.lock();
            let s = st.steps.get(&step).ok_or(UndoError::UnknownStep(step))?;
            if s.committed_ms.is_some() || s.undone {
                return Err(UndoError::BadState {
                    step,
                    reason: "krok już zamknięty".into(),
                });
            }
            s.entries.clone()
        };
        self.revert(step, &entries)
    }

    fn revert(&self, step: StepId, entries: &[JournalEntry]) -> Result<UndoReport, UndoError> {
        let mut failed = Vec::new();
        let mut restored = 0;
        for e in entries.iter().rev() {
            restored += self.restore(e, &mut failed);
        }
        let partial = !failed.is_empty();
        let _ = self.store.append(&JournalRecord::Undone { step, partial });
        if let Some(s) = self.lock().steps.get_mut(&step) {
            s.undone = true;
        }
        let report = UndoReport {
            step,
            restored,
            failed,
        };
        if partial {
            Err(UndoError::Partial(report))
        } else {
            Ok(report)
        }
    }
}
