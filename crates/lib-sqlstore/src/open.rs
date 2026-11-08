//! Otwieranie szyfrowanej bazy i rejestracja sqlite-vec (jedyne `unsafe` w crate).

use std::ffi::{c_char, c_int};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use rusqlite::{Connection, OpenFlags, ffi};

use crate::error::StoreError;
use crate::key::DbKey;

/// Sygnatura punktu wejścia rozszerzenia SQLite (`sqlite3_*_init`).
type ExtensionInit = unsafe extern "C" fn(
    *mut ffi::sqlite3,
    *mut *mut c_char,
    *const ffi::sqlite3_api_routines,
) -> c_int;

/// Kod wyniku jednorazowej rejestracji (ustawiany w `OnceLock`, potem tylko czytany).
static VEC_REGISTRATION: OnceLock<c_int> = OnceLock::new();

/// Rejestruje sqlite-vec jako auto-rozszerzenie dla wszystkich kolejnych połączeń (raz na proces).
///
/// Wywoływane automatycznie przez [`open_connection`]; idempotentne i bezpieczne wątkowo.
pub fn register_sqlite_vec() -> Result<(), StoreError> {
    let rc = *VEC_REGISTRATION.get_or_init(register_once);
    if rc == ffi::SQLITE_OK {
        Ok(())
    } else {
        Err(StoreError::VecRegistration(rc))
    }
}

/// Jedyne `unsafe` w crate (wyjątek od `unsafe_code`, ADR 0008 / spike i).
#[allow(unsafe_code)]
fn register_once() -> c_int {
    // SAFETY: crate `sqlite-vec` deklaruje `sqlite3_vec_init` bez argumentów, ale symbol C ma
    // standardową sygnaturę `sqlite3_*_init(db, pzErrMsg, pApi)` — rzutujemy wskaźnik funkcji na
    // właściwy typ (tak robi test samego crate'a `sqlite-vec`). `sqlite3_auto_extension` jest
    // bezpieczne wątkowo w SQLite; wywołanie następuje raz (w `OnceLock`), przed otwarciem połączeń
    // przez tę bibliotekę.
    unsafe {
        let init = std::mem::transmute::<*const (), ExtensionInit>(
            sqlite_vec::sqlite3_vec_init as *const (),
        );
        ffi::sqlite3_auto_extension(Some(init))
    }
}

/// Otwiera (lub tworzy) szyfrowaną bazę `path` kluczem surowym i ustawia pragmy Alfy.
///
/// Kolejność: rejestracja sqlite-vec → `cipher_log_level = NONE` (SQLCipher nie pisze na stderr) →
/// `PRAGMA key = "x'…'"` → weryfikacja klucza (`sqlite_master`) → `journal_mode = WAL`,
/// `synchronous = NORMAL`, `foreign_keys = ON`, `busy_timeout = 5 s`. Katalog nadrzędny jest tworzony.
/// Zły klucz albo plik niebędący bazą → [`StoreError::NotDatabaseOrWrongKey`].
pub fn open_connection(path: &Path, key: &DbKey) -> Result<Connection, StoreError> {
    register_sqlite_vec()?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags)?;
    conn.execute_batch("PRAGMA cipher_log_level = NONE;")?;
    conn.execute_batch(&key.pragma_sql())?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(StoreError::from_open)?;
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    conn.execute_batch(
        "PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;",
    )?;
    Ok(conn)
}

/// Uchwyt bazy współdzielony między modułami: jedno połączenie za muteksem + ścieżka pliku.
///
/// SQLite w trybie WAL z jednym połączeniem na plik; operacje serializowane przez muteks.
/// Zatrucie muteksu (panika w innym wątku) nie blokuje bazy — transakcje rusqlite i tak
/// wycofują się przy `drop`.
#[derive(Debug)]
pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Db {
    /// Otwiera bazę ([`open_connection`]).
    pub fn open(path: &Path, key: &DbKey) -> Result<Self, StoreError> {
        let conn = open_connection(path, key)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    /// Ścieżka pliku bazy.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Wykonuje `f` z wyłącznym dostępem do połączenia.
    pub fn with<R, E>(&self, f: impl FnOnce(&mut Connection) -> Result<R, E>) -> Result<R, E> {
        let mut guard = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut guard)
    }

    /// Zamyka połączenie (scala WAL); błąd zamknięcia zwraca jako [`StoreError::Sqlite`].
    pub fn close(self) -> Result<(), StoreError> {
        let conn = self
            .conn
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        conn.close().map_err(|(_, e)| StoreError::Sqlite(e))
    }
}
