//! Regresja RUSTSEC-2026-0459 oraz ręczna zgodność starej szyfrowanej bazy z WAL.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_sqlstore::{DbKey, open_connection, rusqlite};
use std::path::{Path, PathBuf};

// Publiczna stała syntetycznej fixture; nie jest kluczem użytkownika ani konfiguracji Alfy.
fn fixture_key() -> DbKey {
    DbKey::from_bytes([42; 32])
}

fn versions(conn: &rusqlite::Connection) -> (String, String) {
    let sqlite = conn
        .query_row("SELECT sqlite_version()", [], |r| r.get(0))
        .unwrap();
    let cipher = conn
        .query_row("PRAGMA cipher_version", [], |r| r.get(0))
        .unwrap();
    (sqlite, cipher)
}

#[test]
fn linked_sqlcipher_uses_fixed_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("security.db");
    let conn = open_connection(&path, &fixture_key()).unwrap();
    let (sqlite, cipher) = versions(&conn);
    let provider: String = conn
        .query_row("PRAGMA cipher_provider", [], |r| r.get(0))
        .unwrap();
    eprintln!("SQLite={sqlite}; SQLCipher={cipher}; provider={provider}");
    assert_eq!(provider.to_ascii_lowercase(), "openssl");
    assert!(!cipher.is_empty(), "SQLCipher musi pozostać włączony");
    assert!(
        rusqlite::version_number() >= 3_054_000,
        "RUSTSEC-2026-0459: SQLite {sqlite}"
    );
    conn.execute_batch(
        "CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES ('syntetyczny zapis');",
    )
    .unwrap();
    conn.close().unwrap();
    assert_ne!(&std::fs::read(path).unwrap()[..16], b"SQLite format 3\0");
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(std::env::var("ALFA_SQLCIPHER_FIXTURE_DIR").expect("ALFA_SQLCIPHER_FIXTURE_DIR"))
}

fn wal(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", path.display()))
}

#[test]
#[ignore = "jednorazowo zachowuje syntetyczną bazę i WAL starego silnika, ALFA_SQLCIPHER_FIXTURE_DIR"]
fn capture_legacy_encrypted_wal_fixture() {
    let target = fixture_dir();
    std::fs::create_dir_all(&target).unwrap();
    assert!(
        !target.join("legacy.db").exists(),
        "nie nadpisuj wcześniejszej fixture"
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.db");
    let conn = open_connection(&path, &fixture_key()).unwrap();
    conn.execute_batch("PRAGMA wal_autocheckpoint=0; CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES ('syntetyczny zapis starego silnika');").unwrap();
    let (sqlite, cipher) = versions(&conn);
    assert!(
        rusqlite::version_number() < 3_054_000,
        "fixture musi pochodzić ze starego silnika"
    );
    std::fs::copy(&path, target.join("legacy.db")).unwrap();
    std::fs::copy(wal(&path), target.join("legacy.db-wal")).unwrap();
    std::fs::write(
        target.join("engine.txt"),
        format!("SQLite={sqlite}; SQLCipher={cipher}\n"),
    )
    .unwrap();
    assert!(
        std::fs::metadata(target.join("legacy.db-wal"))
            .unwrap()
            .len()
            > 32
    );
    conn.close().unwrap();
    eprintln!("fixture: SQLite={sqlite}; SQLCipher={cipher}");
}

#[test]
fn fixed_engine_opens_legacy_encrypted_wal() {
    // Odczyt WAL zmienia bazę i może usunąć WAL: zawsze pracuj na świeżej kopii.
    let (source, source_wal) = match std::env::var("ALFA_SQLCIPHER_FIXTURE_DIR") {
        Ok(dir) => {
            let path = PathBuf::from(dir).join("legacy.db");
            let log = wal(&path);
            (path, log)
        }
        Err(_) => {
            let dir =
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sqlcipher-4.14");
            (dir.join("database.fixture"), dir.join("wal.fixture"))
        }
    };
    assert!(std::fs::metadata(&source_wal).unwrap().len() > 32);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.db");
    std::fs::copy(&source, &path).unwrap();
    std::fs::copy(&source_wal, wal(&path)).unwrap();
    assert_ne!(&std::fs::read(&path).unwrap()[..16], b"SQLite format 3\0");
    let conn = open_connection(&path, &fixture_key()).unwrap();
    let marker: String = conn
        .query_row("SELECT value FROM marker", [], |r| r.get(0))
        .unwrap();
    assert_eq!(marker, "syntetyczny zapis starego silnika");
    let (sqlite, cipher) = versions(&conn);
    eprintln!("odczyt starego WAL: SQLite={sqlite}; SQLCipher={cipher}");
    assert!(rusqlite::version_number() >= 3_054_000);
    assert!(!cipher.is_empty());
}
