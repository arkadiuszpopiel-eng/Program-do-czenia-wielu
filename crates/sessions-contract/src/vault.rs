//! Sejf kluczy baz (Credential Manager/DPAPI w `platform-windows`; tu tylko kontrakt).

use lib_sqlstore::DbKey;

use crate::ids::SessionId;

/// Nazwa klucza bazy-indeksu katalogu sesji (`index.db`).
pub const INDEX_KEY_NAME: &str = "alfa/sessions/index";

/// Nazwa klucza bazy sesji.
pub fn session_key_name(id: &SessionId) -> String {
    format!("alfa/sessions/{id}")
}

/// Błędy sejfu kluczy.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum VaultError {
    /// Sejf niedostępny (np. brak profilu użytkownika, błąd DPAPI).
    #[error("sejf niedostępny: {0}")]
    Unavailable(String),
    /// Zapisany klucz jest uszkodzony.
    #[error("klucz uszkodzony: {0}")]
    Corrupted(String),
    /// Nazwa klucza nieprawidłowa.
    #[error("nieprawidłowa nazwa klucza: {0}")]
    InvalidName(String),
}

/// Sejf kluczy surowych 32 B. Implementacja produkcyjna: Windows Credential Manager
/// (`platform-windows`); atrapa w pamięci: `sessions-fake`; `FileKeyVault` w `sessions-impl`
/// wyłącznie do testów/dev (klucze jawnie na dysku — **niebezpieczne w produkcji**).
///
/// Usunięcie klucza = crypto-shredding bazy (nawet jeśli plik przetrwa, nie da się go odczytać).
pub trait KeyVault: Send + Sync {
    /// Odczytuje klucz; `None`, gdy nie istnieje.
    fn load(&self, name: &str) -> Result<Option<DbKey>, VaultError>;
    /// Zapisuje (nadpisuje) klucz.
    fn store(&self, name: &str, key: &DbKey) -> Result<(), VaultError>;
    /// Usuwa klucz; `true`, jeśli istniał.
    fn delete(&self, name: &str) -> Result<bool, VaultError>;
}

/// Odczytuje klucz albo tworzy i zapisuje nowy losowy klucz.
pub fn load_or_create_key(vault: &dyn KeyVault, name: &str) -> Result<DbKey, VaultError> {
    if let Some(key) = vault.load(name)? {
        return Ok(key);
    }
    let key = DbKey::generate().map_err(|e| VaultError::Unavailable(e.to_string()))?;
    vault.store(name, &key)?;
    Ok(key)
}
