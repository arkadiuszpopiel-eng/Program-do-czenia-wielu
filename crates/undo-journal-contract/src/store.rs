//! Magazyn dziennika: pre-image adresowane treścią (SHA-256, deduplikacja) i rekordy
//! append-only. `-impl` ma wersję katalogową (`undo-store/`), tu wersja w pamięci.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use sha2::{Digest, Sha256};

use crate::types::{BlobId, FileState, JournalRecord};

/// SHA-256 jako hex.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Stan pliku z jego treści.
pub fn state_of(data: &[u8]) -> FileState {
    FileState::Present {
        hash: sha256_hex(data),
        len: data.len() as u64,
    }
}

/// Magazyn dziennika.
pub trait JournalStore: Send + Sync {
    /// Zapisuje pre-image (idempotentnie po hashu).
    fn put_blob(&self, data: &[u8]) -> Result<BlobId, String>;
    /// Odczytuje pre-image (z weryfikacją hasha).
    fn get_blob(&self, id: &BlobId) -> Result<Vec<u8>, String>;
    /// Usuwa pre-image.
    fn delete_blob(&self, id: &BlobId) -> Result<(), String>;
    /// Łączny rozmiar pre-image.
    fn blob_bytes(&self) -> u64;
    /// Dopisuje rekord.
    fn append(&self, record: &JournalRecord) -> Result<(), String>;
    /// Wszystkie rekordy w kolejności zapisu.
    fn load(&self) -> Result<Vec<JournalRecord>, String>;
}

#[derive(Debug, Default)]
struct Mem {
    blobs: BTreeMap<BlobId, Vec<u8>>,
    records: Vec<JournalRecord>,
    fail_append: bool,
}

/// Magazyn w pamięci (atrapa, testy) ze sterowaną awarią zapisu rekordów.
#[derive(Debug, Default)]
pub struct MemStore {
    inner: Mutex<Mem>,
}

impl MemStore {
    fn lock(&self) -> MutexGuard<'_, Mem> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Przełącza awarię `append` (test: operacja bez wpisu nie może zostać).
    pub fn set_fail_append(&self, fail: bool) {
        self.lock().fail_append = fail;
    }

    /// Liczba pre-image.
    pub fn blob_count(&self) -> usize {
        self.lock().blobs.len()
    }
}

impl JournalStore for MemStore {
    fn put_blob(&self, data: &[u8]) -> Result<BlobId, String> {
        let id = BlobId(sha256_hex(data));
        self.lock()
            .blobs
            .entry(id.clone())
            .or_insert_with(|| data.to_vec());
        Ok(id)
    }

    fn get_blob(&self, id: &BlobId) -> Result<Vec<u8>, String> {
        self.lock()
            .blobs
            .get(id)
            .cloned()
            .ok_or_else(|| format!("brak pre-image {}", id.0))
    }

    fn delete_blob(&self, id: &BlobId) -> Result<(), String> {
        self.lock().blobs.remove(id);
        Ok(())
    }

    fn blob_bytes(&self) -> u64 {
        self.lock().blobs.values().map(|b| b.len() as u64).sum()
    }

    fn append(&self, record: &JournalRecord) -> Result<(), String> {
        let mut m = self.lock();
        if m.fail_append {
            return Err("zapis dziennika niedostępny (test)".into());
        }
        m.records.push(record.clone());
        Ok(())
    }

    fn load(&self) -> Result<Vec<JournalRecord>, String> {
        Ok(self.lock().records.clone())
    }
}
