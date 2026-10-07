//! Spike (i): SQLCipher + sqlite-vec + FTS5 w jednym pliku SQLite (ADR 0008, docs/PLAN.md §16.2 F0).
//!
//! Crate tymczasowy (dowód wykonalności), nie moduł Alfy. Jedyne `unsafe` to rejestracja
//! rozszerzenia sqlite-vec przez `sqlite3_auto_extension` (FFI) w [`register_sqlite_vec`].

use std::ffi::{c_char, c_int};
use std::path::Path;
use std::sync::Once;

use rusqlite::{Connection, OpenFlags, ffi, params};

/// Liczba wymiarów embeddingu w spike'u (mała, żeby testy były szybkie).
pub const DIMS: usize = 8;

/// Błędy spike'u.
#[derive(Debug, thiserror::Error)]
pub enum SpikeError {
    /// Błąd SQLite/SQLCipher/sqlite-vec.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// `sqlite3_auto_extension` zwróciło kod błędu.
    #[error("rejestracja sqlite-vec nie powiodła się (kod {0})")]
    Register(i32),
}

/// Sygnatura punktu wejścia rozszerzenia SQLite (`sqlite3_*_init`).
type ExtensionInit = unsafe extern "C" fn(
    *mut ffi::sqlite3,
    *mut *mut c_char,
    *const ffi::sqlite3_api_routines,
) -> c_int;

static REGISTER: Once = Once::new();
static mut REGISTER_RC: c_int = 0;

/// Rejestruje sqlite-vec jako auto-rozszerzenie dla wszystkich kolejnych połączeń (raz na proces).
///
/// Musi być wywołane przed pierwszym `Connection::open*`. Idempotentne.
pub fn register_sqlite_vec() -> Result<(), SpikeError> {
    REGISTER.call_once(|| {
        // SAFETY: `sqlite3_vec_init` w crate `sqlite-vec` jest zadeklarowane bez argumentów, ale
        // symbol C ma standardową sygnaturę `sqlite3_*_init(db, pzErrMsg, pApi)`; rzutujemy wskaźnik
        // funkcji na właściwy typ (tak robi test w samym crate `sqlite-vec`). Wywołanie odbywa się
        // w `Once`, przed otwarciem jakiegokolwiek połączenia.
        let rc = unsafe {
            let init: ExtensionInit = std::mem::transmute::<*const (), ExtensionInit>(
                sqlite_vec::sqlite3_vec_init as *const (),
            );
            let rc = ffi::sqlite3_auto_extension(Some(init));
            REGISTER_RC = rc;
            rc
        };
        let _ = rc;
    });
    // SAFETY: zapis nastąpił w `Once` przed jakimkolwiek odczytem; po `call_once` wartość jest stała.
    let rc = unsafe { REGISTER_RC };
    if rc == ffi::SQLITE_OK {
        Ok(())
    } else {
        Err(SpikeError::Register(rc))
    }
}

/// Otwiera (lub tworzy) bazę pod `path`. `key = None` → bez `PRAGMA key` (test „brak klucza”).
///
/// Po ustawieniu klucza wykonuje zapytanie do `sqlite_master`, żeby błąd złego klucza wystąpił
/// tutaj, a nie przy pierwszym użyciu.
pub fn open(path: &Path, key: Option<&str>) -> Result<Connection, SpikeError> {
    register_sqlite_vec()?;
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
    let conn = Connection::open_with_flags(path, flags)?;
    if let Some(key) = key {
        conn.pragma_update(None, "key", key)?;
    }
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
    Ok(conn)
}

/// Tworzy schemat: `messages`, tabela wektorowa `vec0` i FTS5 (wszystko w tym samym pliku, tryb WAL).
pub fn init_schema(conn: &Connection) -> Result<(), SpikeError> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS messages(
             id INTEGER PRIMARY KEY,
             session_id TEXT NOT NULL,
             text TEXT NOT NULL
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS messages_vec USING vec0(embedding float[{DIMS}]);
         CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
             text, content='messages', content_rowid='id', tokenize='unicode61'
         );"
    ))?;
    Ok(())
}

/// Wiadomość testowa.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// Identyfikator (= rowid w `messages_vec`).
    pub id: i64,
    /// Sesja, do której należy wiadomość.
    pub session_id: String,
    /// Treść (po polsku, z diakrytykami).
    pub text: String,
    /// Embedding deterministyczny ([`embedding_for`]).
    pub embedding: [f32; DIMS],
}

/// Deterministyczny embedding dla `id`: punkt na okręgu w 2 pierwszych wymiarach + „szum” z `id`.
pub fn embedding_for(id: i64) -> [f32; DIMS] {
    let mut v = [0.0_f32; DIMS];
    // `id` jest małe (setki), rzutowanie bez utraty precyzji w praktyce spike'u.
    let angle = (id as f32) * 0.37;
    v[0] = angle.cos();
    v[1] = angle.sin();
    for (i, slot) in v.iter_mut().enumerate().skip(2) {
        *slot = (((id * 31 + i as i64 * 7) % 97) as f32) / 97.0;
    }
    v
}

/// Generuje `n` wiadomości dla `session_id`; co 10. zawiera „żółć”, każda zawiera „sesja”.
pub fn sample_messages(session_id: &str, n: usize) -> Vec<Message> {
    (1..=n)
        .map(|i| {
            let id = i as i64;
            let text = if i % 10 == 0 {
                format!("Wiadomość {i}: sesja {session_id}, kolor żółć i gęś jaźń")
            } else {
                format!("Wiadomość {i}: sesja {session_id}, zwykła treść bez koloru")
            };
            Message {
                id,
                session_id: session_id.to_owned(),
                text,
                embedding: embedding_for(id),
            }
        })
        .collect()
}

fn embedding_bytes(v: &[f32; DIMS]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Wstawia wiadomości do trzech tabel w jednej transakcji.
pub fn insert_messages(conn: &mut Connection, messages: &[Message]) -> Result<(), SpikeError> {
    let tx = conn.transaction()?;
    {
        let mut ins_msg =
            tx.prepare("INSERT INTO messages(id, session_id, text) VALUES (?1, ?2, ?3)")?;
        let mut ins_vec =
            tx.prepare("INSERT INTO messages_vec(rowid, embedding) VALUES (?1, ?2)")?;
        let mut ins_fts = tx.prepare("INSERT INTO messages_fts(rowid, text) VALUES (?1, ?2)")?;
        for m in messages {
            ins_msg.execute(params![m.id, m.session_id, m.text])?;
            ins_vec.execute(params![m.id, embedding_bytes(&m.embedding)])?;
            ins_fts.execute(params![m.id, m.text])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// kNN: `k` najbliższych `id` (z dystansem L2) do `query` przez `MATCH` na tabeli `vec0`.
pub fn knn(
    conn: &Connection,
    query: &[f32; DIMS],
    k: usize,
) -> Result<Vec<(i64, f64)>, SpikeError> {
    let mut stmt = conn.prepare(
        "SELECT rowid, distance FROM messages_vec WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance",
    )?;
    let rows = stmt.query_map(params![embedding_bytes(query), k as i64], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// FTS5: identyfikatory wiadomości pasujących do `query` (składnia FTS5), wg rangi.
pub fn fts_search(conn: &Connection, query: &str) -> Result<Vec<i64>, SpikeError> {
    let mut stmt =
        conn.prepare("SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?1 ORDER BY rank")?;
    let rows = stmt.query_map(params![query], |r| r.get::<_, i64>(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Wersje komponentów (do raportu).
pub fn versions(conn: &Connection) -> Result<(String, String, String), SpikeError> {
    let sqlite: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
    let cipher: String = conn.query_row("PRAGMA cipher_version", [], |r| r.get(0))?;
    let vec: String = conn.query_row("SELECT vec_version()", [], |r| r.get(0))?;
    Ok((sqlite, cipher, vec))
}

/// Usuwa plik bazy razem z `-wal` i `-shm` (crypto-shredding per sesja = usunięcie pliku + klucza).
pub fn remove_database(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)?;
    for suffix in ["-wal", "-shm"] {
        let mut side = path.as_os_str().to_owned();
        side.push(suffix);
        match std::fs::remove_file(side) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
