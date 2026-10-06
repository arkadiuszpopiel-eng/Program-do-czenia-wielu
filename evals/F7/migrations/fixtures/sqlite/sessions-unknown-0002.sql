-- Stan bazy sprzed migracji (wykonać na pustej bazie SQLCipher otwartej kluczem testowym).
-- Baza zapisana przez NOWSZĄ wersję aplikacji (migracja `sessions` 0002, której ta wersja nie zna) —
-- scenariusz powrotu do starszej wersji przez aktualizator (wersje obok siebie, ADR 0007).
CREATE TABLE schema_migrations(
    namespace TEXT NOT NULL,
    version TEXT NOT NULL,
    applied_at INTEGER NOT NULL,
    PRIMARY KEY(namespace, version)
) WITHOUT ROWID;
CREATE TABLE branches(
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
        BEGIN SELECT RAISE(ABORT, 'turn_heard: append-only'); END;

INSERT INTO schema_migrations VALUES ('sessions', '0001', 1780300800000);
INSERT INTO schema_migrations VALUES ('sessions', '0002', 1782892800000);
CREATE TABLE turn_reactions(turn_id INTEGER PRIMARY KEY, emoji TEXT NOT NULL);
