//! Fala 5, m-23 (`evals/F7/migrations`), ADR 0007: powrót do starszej wersji programu po migracji
//! wykonanej przez nowszą. Baza z migracjami nieznanymi kodowi, ale **nowszymi** od każdej znanej,
//! otwiera się bez zmian schematu: w trybie tylko do odczytu (`PRAGMA query_only`), a gdy nowsza
//! wersja oznaczyła migracje jako addytywne (`schema_compat`) — normalnie. Luka/rozwidlenie
//! historii migracji nadal = odmowa (fail-closed).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_sqlstore::rusqlite::{Connection, params};
use lib_sqlstore::{DbKey, MigrationReport, StoreError, migrate, migrate_with, open_connection};

/// Migracje „nowszej wersji programu”: 0002 dokłada tabelę (jak `search` 0002).
const NEWER: &[(&str, &str)] = &[
    (
        "0001",
        "CREATE TABLE turns(id INTEGER PRIMARY KEY, body TEXT NOT NULL);",
    ),
    (
        "0002",
        "CREATE TABLE reactions(turn_id INTEGER PRIMARY KEY, emoji TEXT NOT NULL);",
    ),
];

fn schema_objects(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))
        .unwrap()
}

fn db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_connection(&dir.path().join("r.db"), &DbKey::generate().unwrap()).unwrap();
    (dir, conn)
}

#[test]
fn newer_database_opens_read_only_and_unchanged_in_older_code() {
    let (_dir, conn) = db();
    migrate(&conn, "sessions", NEWER).unwrap();
    conn.execute("INSERT INTO turns(id, body) VALUES (1, 'Cześć')", [])
        .unwrap();
    let before = schema_objects(&conn);
    // Starsza wersja programu zna tylko 0001.
    let report = migrate(&conn, "sessions", &NEWER[..1]);
    assert!(
        report.is_ok(),
        "rollback nie może odciąć danych: {report:?}"
    );
    let body: String = conn
        .query_row("SELECT body FROM turns WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(body, "Cześć", "dane czytelne");
    let write = conn.execute(
        "INSERT INTO turns(id, body) VALUES (?1, ?2)",
        params![2, "nowa tura"],
    );
    assert!(write.is_err(), "zapis zablokowany (tylko odczyt)");
    assert_eq!(schema_objects(&conn), before, "schemat bez zmian");
    let count: i64 = conn
        .query_row("SELECT count(*) FROM turns", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        report.unwrap(),
        MigrationReport {
            applied: Vec::new(),
            newer: vec!["0002".into()],
            read_only: true,
        }
    );
    // Ponowne otwarcie tym samym kodem — to samo, nadal bez zapisu.
    assert_eq!(
        migrate(&conn, "sessions", &NEWER[..1]).unwrap().newer,
        vec!["0002"]
    );
    assert_eq!(schema_objects(&conn), before);
}

#[test]
fn additive_newer_migration_keeps_older_code_read_write() {
    let (_dir, conn) = db();
    // Nowsza wersja oznacza 0002 jako addytywną (tylko nowa tabela).
    let applied = migrate_with(&conn, "sessions", NEWER, &["0002"]).unwrap();
    assert_eq!(applied.applied, vec!["0001", "0002"]);
    assert!(!applied.read_only);
    let report = migrate(&conn, "sessions", &NEWER[..1]).unwrap();
    assert_eq!(report.newer, vec!["0002"]);
    assert!(
        !report.read_only,
        "addytywna → starsza wersja pracuje normalnie"
    );
    conn.execute("INSERT INTO turns(id, body) VALUES (1, 'po powrocie')", [])
        .unwrap();
    // Po ponownej aktualizacji nowsza wersja nic nie stosuje i widzi wiersz starszej.
    assert!(
        migrate(&conn, "sessions", NEWER)
            .unwrap()
            .applied
            .is_empty()
    );
    let n: i64 = conn
        .query_row("SELECT count(*) FROM turns", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn mixed_additive_and_breaking_newer_migrations_mean_read_only() {
    let (_dir, conn) = db();
    let mut v3 = NEWER.to_vec();
    v3.push((
        "0003",
        "ALTER TABLE turns ADD COLUMN lang TEXT NOT NULL DEFAULT 'pl';",
    ));
    migrate_with(&conn, "sessions", &v3, &["0002"]).unwrap();
    let report = migrate(&conn, "sessions", &NEWER[..1]).unwrap();
    assert_eq!(report.newer, vec!["0002", "0003"]);
    assert!(report.read_only);
}

#[test]
fn read_only_connection_refuses_pending_migrations_of_other_namespaces() {
    let (_dir, conn) = db();
    migrate(&conn, "sessions", NEWER).unwrap();
    assert!(migrate(&conn, "sessions", &NEWER[..1]).unwrap().read_only);
    let before = schema_objects(&conn);
    // Inny moduł na tym samym połączeniu (np. `memory` w bazie sesji) z nową migracją.
    let other = migrate(&conn, "memory", &[("0001", "CREATE TABLE memory(x);")]);
    assert!(
        matches!(&other, Err(StoreError::ReadOnly { namespace, version }) if namespace == "memory" && version == "0001"),
        "{other:?}"
    );
    assert!(other.unwrap_err().to_string().contains("tylko do odczytu"));
    assert_eq!(schema_objects(&conn), before);
    // Przestrzeń bez oczekujących migracji — `Ok`, z flagą stanu połączenia.
    assert!(migrate(&conn, "empty", &[]).unwrap().read_only);
}

#[test]
fn additive_list_must_name_known_steps() {
    let (_dir, conn) = db();
    assert!(matches!(
        migrate_with(&conn, "m", NEWER, &["0009"]),
        Err(StoreError::InvalidMigrations { .. })
    ));
}

#[test]
fn gap_in_migration_history_is_still_refused() {
    let (_dir, conn) = db();
    migrate(&conn, "m", NEWER).unwrap();
    // Kod zna 0000 i 0002, a baza ma 0001 — nieznana wersja „w środku”, nie nowsza: odmowa.
    let forked = [("0000", "SELECT 1;"), NEWER[1]];
    assert!(matches!(
        migrate(&conn, "m", &forked),
        Err(StoreError::UnknownMigration { version, .. }) if version == "0001"
    ));
    // Nowsza wersja nieznana, a znana oczekująca (historia niespójna) — odmowa.
    let (_dir2, other) = db();
    migrate(&other, "m", &[NEWER[1]]).unwrap();
    other
        .execute("INSERT INTO schema_migrations VALUES ('m', '0009', 0)", [])
        .unwrap();
    assert!(matches!(
        migrate(&other, "m", NEWER),
        Err(StoreError::MigrationOutOfOrder { version, applied, .. })
            if version == "0001" && applied == "0009"
    ));
}
