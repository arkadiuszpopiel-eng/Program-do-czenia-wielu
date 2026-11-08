//! Magazyn katalogowy `undo-store/`: `blobs/<sha256>` (zapis atomowy, weryfikacja hasha przy
//! odczycie) i `journal.ndjson` (append-only; urwana ostatnia linia po awarii jest pomijana,
//! uszkodzenie w środku = błąd). Szyfrowanie pre-image kluczem sesji — poza zakresem części 1.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use undo_journal_contract::{BlobId, JournalRecord, JournalStore, sha256_hex};

/// Magazyn w katalogu.
#[derive(Debug)]
pub struct DirStore {
    dir: PathBuf,
    bytes: AtomicU64,
    file: Mutex<Option<File>>,
}

fn io(what: &str, e: impl std::fmt::Display) -> String {
    format!("{what}: {e}")
}

fn valid_id(id: &BlobId) -> bool {
    id.0.len() == 64
        && id
            .0
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

impl DirStore {
    /// Otwiera (tworzy) magazyn i liczy rozmiar pre-image.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, String> {
        let dir = dir.into();
        fs::create_dir_all(dir.join("blobs")).map_err(|e| io("katalog undo-store", e))?;
        let mut bytes = 0;
        for entry in fs::read_dir(dir.join("blobs")).map_err(|e| io("blobs", e))? {
            let entry = entry.map_err(|e| io("blobs", e))?;
            bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
        Ok(Self {
            dir,
            bytes: AtomicU64::new(bytes),
            file: Mutex::new(None),
        })
    }

    fn blob_path(&self, id: &BlobId) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err(format!("niepoprawny identyfikator pre-image `{}`", id.0));
        }
        Ok(self.dir.join("blobs").join(&id.0))
    }

    /// Katalog magazynu.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl JournalStore for DirStore {
    fn put_blob(&self, data: &[u8]) -> Result<BlobId, String> {
        let id = BlobId(sha256_hex(data));
        let path = self.blob_path(&id)?;
        if path.exists() {
            return Ok(id);
        }
        let tmp = path.with_extension("tmp");
        let mut f = File::create(&tmp).map_err(|e| io("pre-image", e))?;
        f.write_all(data)
            .and_then(|()| f.sync_all())
            .map_err(|e| io("zapis pre-image", e))?;
        fs::rename(&tmp, &path).map_err(|e| io("podmiana pre-image", e))?;
        self.bytes.fetch_add(data.len() as u64, Ordering::SeqCst);
        Ok(id)
    }

    fn get_blob(&self, id: &BlobId) -> Result<Vec<u8>, String> {
        let data = fs::read(self.blob_path(id)?).map_err(|e| io("odczyt pre-image", e))?;
        if sha256_hex(&data) != id.0 {
            return Err(format!("pre-image {} uszkodzone (hash)", id.0));
        }
        Ok(data)
    }

    fn delete_blob(&self, id: &BlobId) -> Result<(), String> {
        let path = self.blob_path(id)?;
        let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        match fs::remove_file(&path) {
            Ok(()) => {
                self.bytes
                    .fetch_sub(len.min(self.bytes.load(Ordering::SeqCst)), Ordering::SeqCst);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io("usunięcie pre-image", e)),
        }
    }

    fn blob_bytes(&self) -> u64 {
        self.bytes.load(Ordering::SeqCst)
    }

    fn append(&self, record: &JournalRecord) -> Result<(), String> {
        let mut line = serde_json::to_string(record).map_err(|e| io("serializacja", e))?;
        line.push('\n');
        let mut guard = self.file.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.dir.join("journal.ndjson"))
                .map_err(|e| io("dziennik", e))?;
            *guard = Some(f);
        }
        let f = guard.as_mut().ok_or("brak pliku dziennika")?;
        f.write_all(line.as_bytes())
            .and_then(|()| f.sync_data())
            .map_err(|e| io("zapis dziennika", e))
    }

    fn load(&self) -> Result<Vec<JournalRecord>, String> {
        let text = match fs::read_to_string(self.dir.join("journal.ndjson")) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io("odczyt dziennika", e)),
        };
        let complete = text.ends_with('\n');
        let lines: Vec<&str> = text.lines().collect();
        let mut out = Vec::with_capacity(lines.len());
        for (i, line) in lines.iter().enumerate() {
            match serde_json::from_str(line) {
                Ok(r) => out.push(r),
                // Urwana ostatnia linia (awaria w trakcie zapisu) — operacja nie została
                // potwierdzona wpisem, więc jej nie odtwarzamy.
                Err(_) if i + 1 == lines.len() && !complete => {}
                Err(e) => return Err(format!("dziennik uszkodzony w linii {}: {e}", i + 1)),
            }
        }
        Ok(out)
    }
}
