//! Dziennik kosztów w pliku NDJSON (jeden rekord JSON na linię, tylko dopisywanie).
//!
//! Dlaczego NDJSON, a nie SQLite: koszty nie są tajne (bez szyfrowania SQLCipher), zapis jest
//! append-only i zgodny ze strumieniem ModelCalls (`core-log`), plik jest czytelny dla człowieka
//! i narzędzi, nie wymaga budowania SQLite/OpenSSL, a agregaty i tak są w pamięci (odtwarzane
//! z rekordów przy starcie). Przy ~200 B/rekord 10 tys. wywołań/miesiąc to ~2 MB/miesiąc.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cost_meter_contract::{CostError, CostRecord, LedgerStore, LoadedLedger};

/// Dziennik NDJSON.
#[derive(Debug)]
pub struct NdjsonLedger {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

fn storage(e: impl std::fmt::Display) -> CostError {
    CostError::Storage(e.to_string())
}

impl NdjsonLedger {
    /// Dziennik w podanym pliku (tworzony przy pierwszym zapisie).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            file: Mutex::new(None),
        }
    }

    /// Ścieżka pliku.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Otwiera plik do dopisywania; jeśli ostatnia linia jest urwana (awaria w trakcie zapisu),
    /// dopisuje `\n`, żeby nowy rekord nie skleił się z uszkodzonym.
    fn open(&self) -> Result<File, CostError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(storage)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&self.path)
            .map_err(storage)?;
        let len = file.metadata().map_err(storage)?.len();
        if len > 0 {
            let mut last = [0u8; 1];
            file.seek(SeekFrom::Start(len - 1)).map_err(storage)?;
            file.read_exact(&mut last).map_err(storage)?;
            if last[0] != b'\n' {
                file.write_all(b"\n").map_err(storage)?;
            }
        }
        Ok(file)
    }
}

impl LedgerStore for NdjsonLedger {
    fn append(&self, record: &CostRecord) -> Result<(), CostError> {
        let mut line = serde_json::to_vec(record).map_err(storage)?;
        line.push(b'\n');
        let mut guard = self.file.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            *guard = Some(self.open()?);
        }
        let file = guard
            .as_mut()
            .ok_or_else(|| storage("plik dziennika niedostępny"))?;
        let written = file.write_all(&line).and_then(|()| file.flush());
        if let Err(e) = written {
            // Następny zapis otworzy plik od nowa (i domknie ewentualnie urwaną linię).
            *guard = None;
            return Err(storage(e));
        }
        Ok(())
    }

    fn load(&self) -> Result<LoadedLedger, CostError> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LoadedLedger::default());
            }
            Err(e) => return Err(storage(e)),
        };
        let mut loaded = LoadedLedger::default();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<CostRecord>(line) {
                Ok(r) => loaded.records.push(r),
                Err(_) => loaded.skipped_lines += 1,
            }
        }
        Ok(loaded)
    }
}
