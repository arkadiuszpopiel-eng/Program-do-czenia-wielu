//! Dowody spike'u (i): szyfrowanie + kNN + FTS5 w jednym pliku, crypto-shredding per sesja.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use spike_data::{
    embedding_for, fts_search, init_schema, insert_messages, knn, open, remove_database,
    sample_messages, versions, DIMS,
};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Osobny katalog per test (testy biegną równolegle).
fn temp_db(name: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("spike-data-test-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}.db"))
}

fn cleanup(path: &Path) {
    let _ = remove_database(path);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

const KEY: &str = "tajny-klucz-sesji";

#[test]
fn all_three_in_one_file_versions() {
    let path = temp_db("wersje");
    let conn = open(&path, Some(KEY)).unwrap();
    init_schema(&conn).unwrap();
    let (sqlite, cipher, vec) = versions(&conn).unwrap();
    assert!(sqlite.starts_with("3."), "sqlite {sqlite}");
    assert!(cipher.starts_with("4."), "sqlcipher {cipher}");
    assert!(vec.starts_with('v'), "sqlite-vec {vec}");
    let fts5: i64 = conn
        .query_row("SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(fts5, 1, "FTS5 wkompilowane w bundle SQLCipher");
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    drop(conn);
    cleanup(&path);
}

#[test]
fn knn_and_fts_over_100_messages() {
    let path = temp_db("knn-fts");
    let mut conn = open(&path, Some(KEY)).unwrap();
    init_schema(&conn).unwrap();
    let messages = sample_messages("s1", 100);

    let t = Instant::now();
    insert_messages(&mut conn, &messages).unwrap();
    let insert_time = t.elapsed();

    let count: i64 = conn
        .query_row("SELECT count(*) FROM messages", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 100);

    // kNN: najbliższy wektorowi wiadomości 42 jest sama wiadomość 42 (dystans 0).
    let t = Instant::now();
    let res = knn(&conn, &embedding_for(42), 5).unwrap();
    let knn_time = t.elapsed();
    assert_eq!(res.len(), 5);
    assert_eq!(res[0].0, 42);
    assert!(res[0].1.abs() < 1e-6, "dystans {}", res[0].1);
    assert!(
        res.windows(2).all(|w| w[0].1 <= w[1].1),
        "posortowane po dystansie"
    );

    // Zapytanie „między” dwoma wektorami nadal zwraca k wyników.
    let mut q = [0.0_f32; DIMS];
    for (i, slot) in q.iter_mut().enumerate() {
        *slot = (embedding_for(1)[i] + embedding_for(2)[i]) / 2.0;
    }
    let res = knn(&conn, &q, 3).unwrap();
    assert_eq!(res.len(), 3);

    // FTS5 po polsku z diakrytykami.
    let t = Instant::now();
    let zolc = fts_search(&conn, "żółć").unwrap();
    let fts_time = t.elapsed();
    assert_eq!(zolc.len(), 10, "co 10. wiadomość ma „żółć”");
    assert!(zolc.iter().all(|id| id % 10 == 0));
    let sesja = fts_search(&conn, "sesja").unwrap();
    assert_eq!(sesja.len(), 100);
    let jazn = fts_search(&conn, "jaźń").unwrap();
    assert_eq!(jazn.len(), 10);
    let fraza = fts_search(&conn, "\"kolor żółć\"").unwrap();
    assert_eq!(fraza.len(), 10);
    assert!(fts_search(&conn, "fioletowy").unwrap().is_empty());

    eprintln!("100 wstawień: {insert_time:?}; kNN k=5: {knn_time:?}; FTS5: {fts_time:?}");
    drop(conn);

    let size = std::fs::metadata(&path).unwrap().len();
    eprintln!("rozmiar pliku: {size} B");
    assert!(size > 0);
    assert!(
        size < 2 * 1024 * 1024,
        "100 rekordów nie powinno ważyć > 2 MiB: {size}"
    );
    cleanup(&path);
}

#[test]
fn opening_without_key_or_with_wrong_key_fails() {
    let path = temp_db("klucz");
    {
        let mut conn = open(&path, Some(KEY)).unwrap();
        init_schema(&conn).unwrap();
        insert_messages(&mut conn, &sample_messages("s1", 100)).unwrap();
    }
    // Nagłówek jawnej bazy SQLite to „SQLite format 3\0” — tu nie może wystąpić.
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() > 16);
    assert_ne!(
        &bytes[..16],
        b"SQLite format 3\0",
        "plik nie jest jawnym SQLite"
    );
    assert!(
        !bytes.windows(8).any(|w| w == "Wiadomo".as_bytes()),
        "treść nie leży jawnie w pliku"
    );

    let err = open(&path, None).expect_err("bez klucza musi być błąd");
    let msg = err.to_string();
    assert!(msg.contains("not a database"), "bez klucza: {msg}");

    let err = open(&path, Some("zly-klucz")).expect_err("zły klucz musi być błędem");
    let msg = err.to_string();
    assert!(msg.contains("not a database"), "zły klucz: {msg}");

    // Dobry klucz nadal działa i dane są kompletne (w tym FTS i wektory).
    let conn = open(&path, Some(KEY)).unwrap();
    assert_eq!(fts_search(&conn, "żółć").unwrap().len(), 10);
    assert_eq!(knn(&conn, &embedding_for(7), 1).unwrap()[0].0, 7);
    drop(conn);
    cleanup(&path);
}

#[test]
fn crypto_shredding_per_session_is_file_removal() {
    let a = temp_db("sesja-a");
    let b = a.with_file_name("sesja-b.db");
    let (key_a, key_b) = ("klucz-a", "klucz-b");
    for (path, key, sid) in [(&a, key_a, "a"), (&b, key_b, "b")] {
        let mut conn = open(path, Some(key)).unwrap();
        init_schema(&conn).unwrap();
        insert_messages(&mut conn, &sample_messages(sid, 100)).unwrap();
    }
    // Klucz sesji A nie otwiera sesji B (izolacja kryptograficzna, nie tylko logiczna).
    assert!(open(&b, Some(key_a)).is_err());

    // „forget” sesji A = usunięcie pliku (klucz i tak jest per sesja) — kasuje też wektory i FTS.
    remove_database(&a).unwrap();
    assert!(!a.exists());
    assert!(!a.with_extension("db-wal").exists());
    // Ponowne „otwarcie” tworzy pustą bazę — bez śladu po danych.
    let conn = open(&a, Some(key_a)).unwrap();
    let tables: i64 = conn
        .query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tables, 0);
    drop(conn);

    // Sesja B nietknięta.
    let conn = open(&b, Some(key_b)).unwrap();
    assert_eq!(fts_search(&conn, "sesja").unwrap().len(), 100);
    drop(conn);
    cleanup(&a);
    let _ = remove_database(&b);
}
