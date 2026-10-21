//! Snapshot zakresu dla shella: kopia plików zakresu (pre-image z deduplikacją) z limitem
//! liczby plików i rozmiaru. Prostsze i bezpieczniejsze niż shadow-git: brak zewnętrznego
//! narzędzia, przywracanie tymi samymi operacjami `FsPort`, konflikt wykrywany manifestem.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use platform_contract::{OpReceipt, PlatformError};

use crate::engine::Journal;
use crate::store::sha256_hex;
use crate::types::{Manifest, StepId, UndoError, UndoOp};

/// Katalogi zależności i pamięci podręcznej pomijane w snapshocie zakresu (zawsze odtwarzalne
/// z projektu). Bez tego `cargo build`, `git` czy `pnpm install` z korzenia repozytorium
/// zawsze przekraczały limit i polecenie kończyło się odmową (przegląd 2026-10-21).
pub const SNAPSHOT_SKIPPED_DIRS: &[&str] = &["node_modules", "__pycache__", ".venv"];

/// Katalog wyników `cargo` — pomijany tylko obok `Cargo.toml` (gdzie indziej `target` może
/// być zwykłym folderem użytkownika).
pub const SNAPSHOT_CARGO_TARGET: &str = "target";

/// Czy katalog pominąć w snapshocie ([`SNAPSHOT_SKIPPED_DIRS`], `target` obok `Cargo.toml`).
pub fn snapshot_skips(dir: &Path, cargo_project: bool) -> bool {
    let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    SNAPSHOT_SKIPPED_DIRS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(name))
        || (cargo_project && name.eq_ignore_ascii_case(SNAPSHOT_CARGO_TARGET))
}

impl Journal {
    /// Pliki pod `root` (rekurencyjnie, bez katalogów z [`snapshot_skips`]): ścieżka → treść.
    /// Brak katalogu = pusto.
    pub(crate) fn scan(&self, root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, UndoError> {
        let mut out = BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        let mut bytes = 0u64;
        while let Some(dir) = stack.pop() {
            let entries = match self.fs.list_dir(&dir) {
                Ok(e) => e,
                Err(PlatformError::NotFound(_)) => continue,
                Err(e) => return Err(UndoError::Platform(e)),
            };
            let cargo_project = entries.iter().any(|e| {
                !e.is_dir
                    && e.path
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("Cargo.toml"))
            });
            for e in entries {
                if e.is_dir {
                    if !snapshot_skips(&e.path, cargo_project) {
                        stack.push(e.path);
                    }
                    continue;
                }
                let data = self.fs.read(&e.path).map_err(UndoError::Platform)?;
                bytes = bytes.saturating_add(data.len() as u64);
                out.insert(e.path, data);
                if out.len() > self.limits.snapshot_max_files
                    || bytes > self.limits.snapshot_max_bytes
                {
                    return Err(UndoError::SnapshotTooLarge {
                        root: root.to_path_buf(),
                        files: out.len(),
                        bytes,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Manifest stanu zakresu (ścieżka → hash) — stan „po” do wykrywania konfliktów.
    pub(crate) fn hashes(&self, root: &Path) -> Result<BTreeMap<PathBuf, String>, UndoError> {
        Ok(self
            .scan(root)?
            .into_iter()
            .map(|(p, d)| (p, sha256_hex(&d)))
            .collect())
    }

    /// Snapshot zakresu przed poleceniem powłoki (ACC-F3-undo-journal-02).
    pub fn snapshot_scope(&self, step: StepId, root: &Path) -> Result<(), UndoError> {
        let ctx = self.open_ctx(step)?;
        let files = self.scan(root)?;
        let mut manifest: Manifest = BTreeMap::new();
        for (path, data) in files {
            let blob = self.keep_pre_image(&ctx, &path, &data)?.ok_or_else(|| {
                UndoError::SnapshotTooLarge {
                    root: root.to_path_buf(),
                    files: 0,
                    bytes: data.len() as u64,
                }
            })?;
            manifest.insert(path, (sha256_hex(&data), blob));
        }
        let op = UndoOp::ScopeSnapshot {
            root: root.to_path_buf(),
            files: manifest,
        };
        let receipt = OpReceipt::irreversible(platform_contract::FsOperation::Read);
        self.record(step, op, &receipt)
    }

    /// Przywraca zakres do stanu snapshotu: pliki nowe → Kosz, zmienione/usunięte → treść.
    pub(crate) fn restore_scope(
        &self,
        root: &Path,
        files: &Manifest,
        failed: &mut Vec<(PathBuf, String)>,
    ) -> u32 {
        let now = match self.hashes(root) {
            Ok(h) => h,
            Err(e) => {
                failed.push((root.to_path_buf(), e.to_string()));
                return 0;
            }
        };
        let mut restored = 0;
        for path in now.keys().filter(|p| !files.contains_key(*p)) {
            match self.fs.delete_to_recycle_bin(path) {
                Ok(_) => restored += 1,
                Err(e) => failed.push((path.clone(), e.to_string())),
            }
        }
        for (path, (hash, blob)) in files {
            if now.get(path) == Some(hash) {
                continue;
            }
            let res = self
                .store
                .get_blob(blob)
                .map_err(|e| e.to_string())
                .and_then(|data| self.fs.write_atomic(path, &data).map_err(|e| e.to_string()));
            match res {
                Ok(_) => restored += 1,
                Err(e) => failed.push((path.clone(), e)),
            }
        }
        restored
    }
}
