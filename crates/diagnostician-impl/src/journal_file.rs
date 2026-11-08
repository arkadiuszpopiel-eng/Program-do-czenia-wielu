//! Dziennik napraw na dysku: NDJSON append-only z `fsync` po każdym wpisie. Niedokończona
//! ostatnia linia (awaria w trakcie zapisu) jest pomijana i raportowana, nie blokuje startu.

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use diagnostician_contract::JournalEntry;

/// Dziennik w pliku.
#[derive(Debug)]
pub struct FileJournal {
    path: PathBuf,
    file: Mutex<File>,
}

/// Wynik otwarcia.
#[derive(Debug)]
pub struct OpenedJournal {
    /// Dziennik do dopisywania.
    pub journal: FileJournal,
    /// Wpisy odczytane.
    pub entries: Vec<JournalEntry>,
    /// Problemy (uszkodzone linie).
    pub problems: Vec<String>,
}

impl FileJournal {
    /// Otwiera (tworzy) dziennik i wczytuje wpisy.
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<OpenedJournal> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut entries = Vec::new();
        let mut problems = Vec::new();
        for (n, line) in text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            match serde_json::from_str::<JournalEntry>(line) {
                Ok(e) => entries.push(e),
                Err(e) => problems.push(format!("linia {}: {e}", n + 1)),
            }
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(OpenedJournal {
            journal: FileJournal {
                path,
                file: Mutex::new(file),
            },
            entries,
            problems,
        })
    }

    /// Ścieżka.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Dopisuje wpis (linia + `fsync`).
    pub fn append(&self, entry: &JournalEntry) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(entry).map_err(std::io::Error::other)?;
        line.push(b'\n');
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        file.write_all(&line)?;
        file.sync_data()
    }
}
