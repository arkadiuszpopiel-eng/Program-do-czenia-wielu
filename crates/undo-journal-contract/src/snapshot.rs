//! Snapshot zakresu dla shella: kopia plików zakresu (pre-image z deduplikacją) z limitem
//! liczby plików i rozmiaru. Prostsze i bezpieczniejsze niż shadow-git: brak zewnętrznego
//! narzędzia, przywracanie tymi samymi operacjami `FsPort`, konflikt wykrywany manifestem.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use platform_contract::{OpReceipt, PlatformError};

use crate::engine::Journal;
use crate::store::sha256_hex;
use crate::types::{Manifest, StepId, UndoError, UndoOp};

impl Journal {
    /// Wszystkie pliki pod `root` (rekurencyjnie): ścieżka → treść. Brak katalogu = pusto.
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
            for e in entries {
                if e.is_dir {
                    stack.push(e.path);
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
