//! Historia diagnostyki: zapisy parametrów (`snapshots.jsonl`) i przebiegi testów (`runs.jsonl`)
//! w katalogu danych lokalnych. Jeden wpis w linii, dopisywanie na końcu; po przekroczeniu
//! [`KEEP`] wpisów plik jest przepisywany z najnowszymi (przez plik tymczasowy). Uszkodzona linia
//! jest pomijana, nie psuje reszty historii.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pc_diag_contract::{CheckResult, CheckRun, DiagError, Snapshot};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Ile wpisów każdego rodzaju zostaje.
pub const KEEP: usize = 200;
/// Plik zapisów parametrów.
pub const SNAPSHOTS: &str = "snapshots.jsonl";
/// Plik przebiegów testów.
pub const RUNS: &str = "runs.jsonl";

/// Historia w katalogu.
pub struct History {
    dir: PathBuf,
    write: Mutex<()>,
}

fn storage(e: impl std::fmt::Display) -> DiagError {
    DiagError::Storage(e.to_string())
}

fn read<T: DeserializeOwned>(path: &Path) -> Vec<T> {
    std::fs::read_to_string(path)
        .map(|t| {
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

impl History {
    /// Historia w katalogu (tworzonym przy pierwszym zapisie).
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            write: Mutex::new(()),
        }
    }

    /// Katalog historii.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn append<T: Serialize + DeserializeOwned>(
        &self,
        file: &str,
        item: &T,
    ) -> Result<(), DiagError> {
        let _guard = self.write.lock().unwrap_or_else(|p| p.into_inner());
        std::fs::create_dir_all(&self.dir).map_err(storage)?;
        let path = self.dir.join(file);
        let line = serde_json::to_string(item).map_err(storage)?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(storage)?;
        writeln!(f, "{line}").map_err(storage)?;
        drop(f);
        let all: Vec<serde_json::Value> = read(&path);
        if all.len() > KEEP + KEEP / 4 {
            let keep = &all[all.len() - KEEP..];
            let text: String = keep.iter().map(|v| format!("{v}\n")).collect();
            let tmp = path.with_extension("jsonl.tmp");
            std::fs::write(&tmp, text).map_err(storage)?;
            std::fs::rename(&tmp, &path).map_err(storage)?;
        }
        Ok(())
    }

    /// Dopisuje zapis parametrów.
    pub fn add_snapshot(&self, s: &Snapshot) -> Result<(), DiagError> {
        self.append(SNAPSHOTS, s)
    }

    /// Dopisuje przebieg testów.
    pub fn add_run(&self, r: &CheckRun) -> Result<(), DiagError> {
        self.append(RUNS, r)
    }

    /// Zapisy parametrów (najnowsze na końcu).
    pub fn snapshots(&self) -> Vec<Snapshot> {
        read(&self.dir.join(SNAPSHOTS))
    }

    /// Przebiegi testów (najnowsze na końcu).
    pub fn runs(&self) -> Vec<CheckRun> {
        read(&self.dir.join(RUNS))
    }

    /// Ostatni niepominięty wynik testu o tym kluczu.
    pub fn last_result(&self, key: &str) -> Option<CheckResult> {
        self.runs().into_iter().rev().find_map(|run| {
            run.results
                .into_iter()
                .find(|r| r.kind.key() == key && r.verdict != pc_diag_contract::Verdict::Skipped)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pc_diag_contract::{CheckKind, Verdict};

    #[test]
    fn appends_trims_and_skips_broken_lines() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::new(dir.path().join("pc"));
        assert!(h.snapshots().is_empty() && h.last_result("memory").is_none());
        for i in 0..(KEEP + KEEP / 4 + 2) {
            let mut r = CheckResult::skipped(CheckKind::Memory, "x");
            if i % 2 == 0 {
                r.verdict = Verdict::Pass;
                r.duration_ms = i as u64;
            }
            h.add_run(&CheckRun::new(format!("t{i}"), vec![r])).unwrap();
        }
        let runs = h.runs();
        assert!(runs.len() <= KEEP + KEEP / 4 && runs.len() >= KEEP);
        assert_eq!(runs.last().unwrap().at, format!("t{}", KEEP + KEEP / 4 + 1));
        let last = h.last_result("memory").unwrap();
        assert_eq!(last.verdict, Verdict::Pass, "pominięte nie są porównywane");
        std::fs::OpenOptions::new()
            .append(true)
            .open(dir.path().join("pc").join(RUNS))
            .unwrap()
            .write_all(b"{zepsute\n")
            .unwrap();
        assert_eq!(h.runs().len(), runs.len());
        h.add_snapshot(&pc_diag_fake::sample()).unwrap();
        assert_eq!(h.snapshots().len(), 1);
    }
}
