//! Wspólna biblioteka danych Alfy (kategoria `lib-*`, crates/README.md; ADR 0008, spike i).
//!
//! Bez logiki modułu. Dostarcza:
//! - [`DbKey`] — klucz surowy 32 B (zerowany przy `drop`), podawany SQLCipher jako `x'…'` bez KDF;
//! - [`Db`] / [`open_connection`] — otwarcie szyfrowanej bazy (`cipher_log_level = NONE`, WAL,
//!   `foreign_keys`), z rejestracją sqlite-vec przez `sqlite3_auto_extension` (jedno `unsafe`);
//! - [`migrate`] — prosty runner migracji (tabela `schema_migrations`, przestrzenie nazw modułów);
//! - [`remove_database`] — usunięcie pliku bazy razem z `-wal`/`-shm`/`-journal` (crypto-shredding);
//! - [`fold_pl`], [`search_tokens`], [`fts5_match`] — normalizacja tekstu PL do FTS5
//!   („zolc” znajduje „żółć”);
//! - [`vector_to_blob`] / [`blob_to_vector`] — format wektora `float[]` dla `vec0`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod error;
mod files;
mod key;
mod migrate;
mod open;
mod text;
mod vector;

pub use error::StoreError;
pub use files::{database_files, remove_database};
pub use key::DbKey;
pub use migrate::{MigrationReport, migrate};
pub use open::{Db, open_connection, register_sqlite_vec};
pub use text::{Token, fold_char, fold_pl, fts5_match, search_tokens, tokenize};
pub use vector::{blob_to_vector, vector_to_blob};

/// Re-eksport `rusqlite` (jedna wersja w całym workspace; moduły używają typów stąd).
pub use rusqlite;

/// Bieżący czas uniksowy w milisekundach (0, jeśli zegar systemowy jest przed epoką).
pub fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}
