//! Schematy baz: katalog `index.db` i baza sesji `<id>.db` (migracje `lib_sqlstore::migrate`).

/// Przestrzeń nazw migracji katalogu.
pub const INDEX_NAMESPACE: &str = "sessions.index";

/// Przestrzeń nazw migracji bazy sesji.
pub const SESSION_NAMESPACE: &str = "sessions";

/// Migracje `index.db`: metadane jako JSON + liczniki listy (tury, nieprzeczytane, ostatnia tura).
pub const INDEX_MIGRATIONS: &[(&str, &str)] = &[(
    "0001",
    "CREATE TABLE sessions(
        id TEXT PRIMARY KEY,
        meta TEXT NOT NULL,
        turns INTEGER NOT NULL DEFAULT 0,
        unread INTEGER NOT NULL DEFAULT 0,
        last_turn_at TEXT
    ) WITHOUT ROWID;",
)];

/// Migracje bazy sesji. Tabele `turns`, `branches` i `turn_heard` są **append-only**: wyzwalacze
/// odrzucają każde `UPDATE`/`DELETE` (obrona w głąb — kod i tak nie ma takich operacji).
/// Usunięcie sesji = usunięcie pliku i klucza, nie wierszy.
pub const SESSION_MIGRATIONS: &[(&str, &str)] = &[(
    "0001",
    "CREATE TABLE branches(
        id INTEGER PRIMARY KEY,
        base_turn_id INTEGER REFERENCES turns(id),
        created_at INTEGER NOT NULL
    );
    CREATE TABLE turns(
        id INTEGER PRIMARY KEY,
        parent_id INTEGER REFERENCES turns(id),
        branch_id INTEGER NOT NULL REFERENCES branches(id),
        role TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        body BLOB NOT NULL
    );
    CREATE INDEX turns_by_parent ON turns(parent_id, id);
    CREATE TABLE turn_heard(
        turn_id INTEGER PRIMARY KEY REFERENCES turns(id),
        chars INTEGER NOT NULL,
        approximate INTEGER NOT NULL,
        recorded_at INTEGER NOT NULL
    );
    CREATE TABLE turn_hidden(
        turn_id INTEGER PRIMARY KEY REFERENCES turns(id),
        hidden_at INTEGER NOT NULL
    );
    CREATE TABLE session_state(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
    CREATE TRIGGER turns_no_update BEFORE UPDATE ON turns
        BEGIN SELECT RAISE(ABORT, 'turns: historia append-only'); END;
    CREATE TRIGGER turns_no_delete BEFORE DELETE ON turns
        BEGIN SELECT RAISE(ABORT, 'turns: historia append-only'); END;
    CREATE TRIGGER branches_no_update BEFORE UPDATE ON branches
        BEGIN SELECT RAISE(ABORT, 'branches: append-only'); END;
    CREATE TRIGGER branches_no_delete BEFORE DELETE ON branches
        BEGIN SELECT RAISE(ABORT, 'branches: append-only'); END;
    CREATE TRIGGER heard_no_update BEFORE UPDATE ON turn_heard
        BEGIN SELECT RAISE(ABORT, 'turn_heard: append-only'); END;
    CREATE TRIGGER heard_no_delete BEFORE DELETE ON turn_heard
        BEGIN SELECT RAISE(ABORT, 'turn_heard: append-only'); END;",
)];

/// Rozmiar pamięci podręcznej stron per baza sesji (KiB) — budżet RAM ≤ 10 MB przy 20 sesjach.
pub const SESSION_CACHE_KIB: i64 = 256;
