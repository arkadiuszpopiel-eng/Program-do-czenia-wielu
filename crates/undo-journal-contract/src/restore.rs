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
                    // Zmiana samej wielkości liter (`readme.md` → `README.md`) na systemie plików
                    // Windows to ten sam wpis: stara nazwa nadal „istnieje", więc oczekiwanie
                    // „źródło nieobecne" dawało konflikt przy każdej próbie cofnięcia.
                    if !case_only_rename(from, to) {
                        out.insert(from.clone(), FileState::Absent);
                    }
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
                let mut post = posts.get(&e.seq).cloned().unwrap_or_default();
                post.retain(|p, _| !self.in_skipped_dir(root, p));
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

    /// Usuwa (do Kosza) katalogi utworzone przez cofaną operację, od najgłębszego; katalog
    /// z jakąkolwiek zawartością zostaje — mógł w nim coś zostawić właściciel albo inna operacja.
    fn remove_created_dirs(&self, dirs: &[PathBuf]) {
        for dir in dirs.iter().rev() {
            match self.fs.list_dir(dir) {
                Ok(entries) if entries.is_empty() => {
                    if self.fs.delete_to_recycle_bin(dir).is_err() {
                        return;
                    }
                }
                _ => return,
            }
        }
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
            Ok(()) => {
                self.remove_created_dirs(&e.created_dirs);
                1
            }
            Err(why) => {
                failed.push((path, why));
                0
            }
        }
    }

    /// Cofa zatwierdzony krok. Konflikt → nic nie jest zmieniane; częściowe niepowodzenie →
    /// `UndoError::Partial` z raportem. Każda przywrócona operacja ma rekord w dzienniku, więc
    /// ponowne cofnięcie (także po restarcie) przywraca tylko to, co nie wróciło.
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
            let left: Vec<JournalEntry> = s
                .entries
                .iter()
                .filter(|e| !s.restored.contains(&e.seq))
                .cloned()
                .collect();
            (left, s.posts.clone())
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
            let before = failed.len();
            restored += self.restore(e, &mut failed);
            if failed.len() == before {
                // Przywrócona — zapis od razu: przerwanie w połowie nie powtórzy tej zmiany.
                self.store
                    .append(&JournalRecord::Restored { step, seq: e.seq })
                    .map_err(UndoError::Store)?;
                if let Some(s) = self.lock().steps.get_mut(&step) {
                    s.restored.insert(e.seq);
                }
            }
        }
        let partial = !failed.is_empty();
        // Błąd zapisu nie jest już przemilczany: stan po restarcie musi zgadzać się z dyskiem.
        self.store
            .append(&JournalRecord::Undone { step, partial })
            .map_err(UndoError::Store)?;
        if let Some(s) = self.lock().steps.get_mut(&step) {
            s.undone = !partial;
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

/// Czy przeniesienie zmienia wyłącznie wielkość liter w ścieżce (na Windows — ten sam wpis katalogu).
fn case_only_rename(from: &std::path::Path, to: &std::path::Path) -> bool {
    cfg!(windows)
        && from != to
        && from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase()
}

#[cfg(test)]
mod case_tests {
    use super::case_only_rename;
    use std::path::Path;

    #[test]
    fn case_only_rename_is_recognised_on_windows_only() {
        let same = case_only_rename(Path::new("C:/a/readme.md"), Path::new("C:/a/README.md"));
        assert_eq!(same, cfg!(windows));
        assert!(!case_only_rename(
            Path::new("C:/a/x.md"),
            Path::new("C:/a/y.md")
        ));
        assert!(!case_only_rename(
            Path::new("C:/a/x.md"),
            Path::new("C:/a/x.md")
        ));
        // Polskie znaki: „Żółw.txt" → „żółw.txt" to też zmiana samej wielkości liter.
        assert_eq!(
            case_only_rename(Path::new("C:/a/Żółw.txt"), Path::new("C:/a/żółw.txt")),
            cfg!(windows)
        );
    }
}
