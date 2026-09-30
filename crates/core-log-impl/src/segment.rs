//! Segmenty NDJSON strumienia: `<root>/<strumień>/<pierwszy seq, 20 cyfr>.ndjson`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use core_bus_contract::Event;
use core_log_contract::{LogError, LogStream};
use serde::{Deserialize, Serialize};

/// Rozszerzenie plików segmentów.
pub const SEGMENT_EXT: &str = "ndjson";

/// Linia segmentu na dysku (NDJSON). `written_at` służy retencji, `event.ts` — zapytaniom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskRecord {
    /// Numer sekwencyjny w strumieniu.
    pub seq: u64,
    /// Wersja schematu zdarzenia w chwili zapisu.
    pub schema_version: u32,
    /// Czas zapisu (zegar log-writera).
    pub written_at: DateTime<Utc>,
    /// Zdarzenie po redakcji.
    pub event: Event,
}

/// Metadane segmentu.
#[derive(Debug, Clone)]
pub struct Segment {
    /// Pierwszy numer sekwencyjny (z nazwy pliku).
    pub first_seq: u64,
    /// Ścieżka pliku.
    pub path: PathBuf,
    /// Rozmiar w bajtach.
    pub bytes: u64,
    /// Czas zapisu ostatniego rekordu (leniwie; `None` = nieodczytany).
    pub last_written: Option<DateTime<Utc>>,
}

/// Nazwa katalogu strumienia (jak w JSON: `model_calls`, `tools_gui`, `voice`, `diagnostics`).
pub fn stream_dir_name(stream: LogStream) -> &'static str {
    match stream {
        LogStream::ModelCalls => "model_calls",
        LogStream::ToolsGui => "tools_gui",
        LogStream::Voice => "voice",
        LogStream::Diagnostics => "diagnostics",
    }
}

/// Ścieżka segmentu zaczynającego się od `first_seq`.
pub fn segment_path(dir: &Path, first_seq: u64) -> PathBuf {
    dir.join(format!("{first_seq:020}.{SEGMENT_EXT}"))
}

/// Mapuje błąd we/wy na `LogError::Io` z kontekstem.
pub fn io_err(context: &str, e: impl std::fmt::Display) -> LogError {
    LogError::Io(format!("{context}: {e}"))
}

/// Segmenty katalogu posortowane po `first_seq` (pliki o innych nazwach są pomijane).
pub fn list_segments(dir: &Path) -> Result<Vec<Segment>, LogError> {
    let mut segments = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| io_err("odczyt katalogu logów", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_err("odczyt katalogu logów", e))?;
        let path = entry.path();
        let parsed = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|_| path.extension().and_then(|e| e.to_str()) == Some(SEGMENT_EXT))
            .and_then(|s| s.parse::<u64>().ok());
        let Some(first_seq) = parsed else {
            continue;
        };
        let bytes = entry
            .metadata()
            .map_err(|e| io_err("metadane segmentu", e))?
            .len();
        segments.push(Segment {
            first_seq,
            path,
            bytes,
            last_written: None,
        });
    }
    segments.sort_by_key(|s| s.first_seq);
    Ok(segments)
}

/// Rekordy segmentu; linie uszkodzone (np. urwany zapis) są pomijane.
pub fn read_records(path: &Path) -> Result<Vec<DiskRecord>, LogError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err("odczyt segmentu", e)),
    };
    Ok(bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_slice::<DiskRecord>(line).ok())
        .collect())
}

/// Czy plik kończy się znakiem nowej linii (pusty = tak). Urwany ogon wymusza nowy segment,
/// bo dopisanie do niego skleiłoby rekordy.
pub fn ends_cleanly(path: &Path) -> Result<bool, LogError> {
    let bytes = fs::read(path).map_err(|e| io_err("odczyt segmentu", e))?;
    Ok(bytes.last().is_none_or(|b| *b == b'\n'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_listing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            segment_path(dir.path(), 42).file_name().unwrap(),
            "00000000000000000042.ndjson"
        );
        fs::write(segment_path(dir.path(), 10), b"x\n").unwrap();
        fs::write(segment_path(dir.path(), 2), b"").unwrap();
        fs::write(dir.path().join("notatka.txt"), b"").unwrap();
        fs::write(dir.path().join("abc.ndjson"), b"").unwrap();
        let segs = list_segments(dir.path()).unwrap();
        let firsts: Vec<u64> = segs.iter().map(|s| s.first_seq).collect();
        assert_eq!(firsts, [2, 10]);
        assert_eq!(segs[1].bytes, 2);
        assert!(ends_cleanly(&segs[0].path).unwrap());
        assert!(read_records(&segs[1].path).unwrap().is_empty());
        assert!(
            read_records(&dir.path().join("brak.ndjson"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(stream_dir_name(LogStream::ToolsGui), "tools_gui");
    }
}
