//! Magazyn wtyczek: rekordy wersji i bajty modułów (adresowane hashem SHA-256). Bajty są
//! weryfikowane hashem przy **każdym** ładowaniu — podmiana pliku po zatwierdzeniu = odmowa.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use crate::model::PluginRecord;
use crate::validate::is_sha256_hex;

/// Magazyn (impl: katalog `%LOCALAPPDATA%\Alfa\plugins`; testy/atrapa: pamięć).
pub trait PluginStore: Send + Sync {
    /// Wszystkie rekordy.
    fn load_records(&self) -> Result<Vec<PluginRecord>, String>;
    /// Zapis rekordów (atomowo).
    fn save_records(&self, records: &[PluginRecord]) -> Result<(), String>;
    /// Zapis bajtów modułu pod hashem.
    fn put_wasm(&self, sha256: &str, bytes: &[u8]) -> Result<(), String>;
    /// Bajty modułu (`None` = brak).
    fn get_wasm(&self, sha256: &str) -> Result<Option<Vec<u8>>, String>;
    /// Usunięcie bajtów modułu.
    fn delete_wasm(&self, sha256: &str) -> Result<(), String>;
}

/// Klucz modułu: tylko kanoniczny SHA-256 (żadnych ścieżek z zewnątrz).
pub fn wasm_key(sha256: &str) -> Result<&str, String> {
    if is_sha256_hex(sha256) {
        Ok(sha256)
    } else {
        Err("klucz modułu musi być SHA-256 (hex)".into())
    }
}

/// Magazyn w pamięci.
#[derive(Debug, Default)]
pub struct MemPluginStore {
    records: Mutex<Vec<PluginRecord>>,
    wasm: Mutex<BTreeMap<String, Vec<u8>>>,
    fail_saves: Mutex<bool>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl MemPluginStore {
    /// Wymusza błąd zapisu rekordów (test: błąd zapisu = brak zmiany).
    pub fn fail_saves(&self, fail: bool) {
        *lock(&self.fail_saves) = fail;
    }

    /// Liczba przechowywanych modułów.
    pub fn wasm_count(&self) -> usize {
        lock(&self.wasm).len()
    }
}

impl PluginStore for MemPluginStore {
    fn load_records(&self) -> Result<Vec<PluginRecord>, String> {
        Ok(lock(&self.records).clone())
    }

    fn save_records(&self, records: &[PluginRecord]) -> Result<(), String> {
        if *lock(&self.fail_saves) {
            return Err("zapis niedostępny (test)".into());
        }
        *lock(&self.records) = records.to_vec();
        Ok(())
    }

    fn put_wasm(&self, sha256: &str, bytes: &[u8]) -> Result<(), String> {
        lock(&self.wasm).insert(wasm_key(sha256)?.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn get_wasm(&self, sha256: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(lock(&self.wasm).get(wasm_key(sha256)?).cloned())
    }

    fn delete_wasm(&self, sha256: &str) -> Result<(), String> {
        lock(&self.wasm).remove(wasm_key(sha256)?);
        Ok(())
    }
}
