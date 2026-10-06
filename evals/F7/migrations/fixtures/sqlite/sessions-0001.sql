-- Stan bazy sprzed migracji (wykonać na pustej bazie SQLCipher otwartej kluczem testowym).
-- Baza sesji po migracji `sessions` 0001 (F1): gałąź, dwie tury, usłyszany prefiks, stan.
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
INSERT INTO branches(id, base_turn_id, created_at) VALUES (1, NULL, 1780300800000);
INSERT INTO turns(id, parent_id, branch_id, role, created_at, body) VALUES (1, NULL, 1, 'user', 1780300801000, CAST('{"role":"user","author":{"kind":"user"},"content":{"text":"Jaka będzie pogoda w Toruniu?","blocks":[]},"usage":null,"created_at":"2026-06-01T10:00:01Z"}' AS BLOB));
INSERT INTO turns(id, parent_id, branch_id, role, created_at, body) VALUES (2, 1, 1, 'assistant', 1780300804000, CAST('{"role":"assistant","author":{"kind":"agent","agent":"alfa"},"content":{"text":"Jutro w Toruniu słonecznie, 24 stopnie.","blocks":[]},"usage":null,"created_at":"2026-06-01T10:00:04Z"}' AS BLOB));
INSERT INTO turn_heard(turn_id, chars, approximate, recorded_at) VALUES (2, 18, 1, 1780300806000);
INSERT INTO session_state(key, value) VALUES ('active_leaf', '2');
