//! Testy integracyjne: szyfrowanie, zły klucz, sqlite-vec, FTS5 z `fold_pl`, migracje, usuwanie plików.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_sqlstore::rusqlite::params;
use lib_sqlstore::{
    Db, DbKey, StoreError, fold_pl, fts5_match, migrate, open_connection, remove_database,
    vector_to_blob,
};

const MARKER: &str = "tajny-znacznik-zolty-kot-8472";

#[test]
fn encrypted_file_rejects_missing_and_wrong_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a").join("s.db");
    let key = DbKey::generate().unwrap();
    {
        let conn = open_connection(&path, &key).unwrap();
        conn.execute_batch("CREATE TABLE t(x TEXT);").unwrap();
        conn.execute("INSERT INTO t VALUES (?1)", params![MARKER])
            .unwrap();
        conn.close().unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    assert_ne!(&bytes[..16], b"SQLite format 3\0");
    assert!(!bytes.windows(MARKER.len()).any(|w| w == MARKER.as_bytes()));

    let wrong = DbKey::generate().unwrap();
    assert!(matches!(
        open_connection(&path, &wrong),
        Err(StoreError::NotDatabaseOrWrongKey)
    ));

    let db = Db::open(&path, &key).unwrap();
    let got: String = db
        .with(|c| c.query_row("SELECT x FROM t", [], |r| r.get(0)))
        .unwrap();
    assert_eq!(got, MARKER);
    assert_eq!(db.path(), path);
    db.close().unwrap();
}

#[test]
fn pragmas_wal_and_foreign_keys_are_set() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_connection(&dir.path().join("p.db"), &DbKey::generate().unwrap()).unwrap();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");
    assert_eq!(fk, 1);
}

#[test]
fn sqlite_vec_and_fts5_with_fold_in_one_encrypted_db() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_connection(&dir.path().join("v.db"), &DbKey::generate().unwrap()).unwrap();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE v USING vec0(embedding float[2] distance_metric=cosine);
         CREATE VIRTUAL TABLE f USING fts5(folded, tokenize='unicode61 remove_diacritics 2');",
    )
    .unwrap();
    for (id, text, vec) in [
        (1_i64, "Kolor żółć i gęś", [1.0_f32, 0.0]),
        (2, "Zwykła treść", [0.0, 1.0]),
    ] {
        conn.execute(
            "INSERT INTO v(rowid, embedding) VALUES (?1, ?2)",
            params![id, vector_to_blob(&vec)],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO f(rowid, folded) VALUES (?1, ?2)",
            params![id, fold_pl(text)],
        )
        .unwrap();
    }
    let nearest: i64 = conn
        .query_row(
            "SELECT rowid FROM v WHERE embedding MATCH ?1 AND k = 1",
            params![vector_to_blob(&[0.9, 0.1])],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(nearest, 1);
    for query in ["zolc", "ŻÓŁĆ", "gęś", "ges", "kolo"] {
        let expr = fts5_match(query).unwrap();
        let hit: i64 = conn
            .query_row("SELECT rowid FROM f WHERE f MATCH ?1", params![expr], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(hit, 1, "zapytanie {query}");
    }
}

const STEPS_V1: &[(&str, &str)] = &[
    ("0001", "CREATE TABLE a(x INTEGER);"),
    ("0002", "CREATE TABLE b(y INTEGER);"),
];

#[test]
fn migrations_apply_once_per_namespace() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_connection(&dir.path().join("m.db"), &DbKey::generate().unwrap()).unwrap();
    let first = migrate(&conn, "mod-a", STEPS_V1).unwrap();
    assert_eq!(first.applied, vec!["0001", "0002"]);
    assert!(
        migrate(&conn, "mod-a", STEPS_V1)
            .unwrap()
            .applied
            .is_empty()
    );
    // Inna przestrzeń nazw nie widzi migracji `mod-a`.
    let other = migrate(&conn, "mod-b", &[("0001", "CREATE TABLE c(z);")]).unwrap();
    assert_eq!(other.applied, vec!["0001"]);
    // Kolejna wersja dochodzi przyrostowo.
    let mut v2 = STEPS_V1.to_vec();
    v2.push(("0003", "CREATE TABLE d(w);"));
    assert_eq!(migrate(&conn, "mod-a", &v2).unwrap().applied, vec!["0003"]);
}

#[test]
fn migration_errors() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_connection(&dir.path().join("e.db"), &DbKey::generate().unwrap()).unwrap();
    migrate(&conn, "m", STEPS_V1).unwrap();
    // Baza nowsza niż kod.
    assert!(matches!(
        migrate(&conn, "m", &STEPS_V1[..1]),
        Err(StoreError::UnknownMigration { .. })
    ));
    // Kolejność i duplikaty.
    assert!(matches!(
        migrate(&conn, "m", &[("0002", "x"), ("0001", "y")]),
        Err(StoreError::InvalidMigrations { .. })
    ));
    assert!(matches!(
        migrate(&conn, "", STEPS_V1),
        Err(StoreError::InvalidMigrations { .. })
    ));
    // Migracja wstawiona „w środek”.
    let mut gap = STEPS_V1.to_vec();
    gap.insert(1, ("0001a", "CREATE TABLE g(x);"));
    assert!(matches!(
        migrate(&conn, "m", &gap),
        Err(StoreError::MigrationOutOfOrder { .. })
    ));
    // Błąd SQL wycofuje migrację w całości (tabela `half` nie powstaje).
    let broken = [("0001", "CREATE TABLE half(x); THIS IS NOT SQL;")];
    assert!(matches!(
        migrate(&conn, "broken", &broken),
        Err(StoreError::MigrationFailed { .. })
    ));
    let exists: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'half'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(exists, 0);
}

#[test]
fn migration_inside_open_transaction_rolls_back_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = open_connection(&dir.path().join("t.db"), &DbKey::generate().unwrap()).unwrap();
    {
        let tx = conn.transaction().unwrap();
        migrate(&tx, "tx", STEPS_V1).unwrap();
        // drop bez commit → wycofanie
    }
    assert_eq!(migrate(&conn, "tx", STEPS_V1).unwrap().applied.len(), 2);
}

#[test]
fn remove_database_deletes_sidecars_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.db");
    let key = DbKey::generate().unwrap();
    let conn = open_connection(&path, &key).unwrap();
    conn.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);")
        .unwrap();
    assert!(dir.path().join("r.db-wal").exists());
    drop(conn);
    std::fs::write(dir.path().join("r.db-shm"), b"x").unwrap();
    let removed = remove_database(&path).unwrap();
    assert!(removed.contains(&path));
    assert!(!path.exists());
    assert!(!dir.path().join("r.db-wal").exists());
    assert!(!dir.path().join("r.db-shm").exists());
    assert!(remove_database(&path).unwrap().is_empty());
    // Po usunięciu ten sam klucz tworzy pustą bazę (dane zniknęły).
    let fresh = open_connection(&path, &key).unwrap();
    let tables: i64 = fresh
        .query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tables, 0);
}
