-- Stan bazy sprzed migracji (wykonać na pustej bazie SQLCipher otwartej kluczem testowym).
-- Indeks wyszukiwania po `search` 0001 (bez tabeli brakujących wektorów z 0002).
CREATE TABLE schema_migrations(
    namespace TEXT NOT NULL,
    version TEXT NOT NULL,
    applied_at INTEGER NOT NULL,
    PRIMARY KEY(namespace, version)
) WITHOUT ROWID;
CREATE TABLE search_docs(
        id INTEGER PRIMARY KEY,
        kind TEXT NOT NULL,
        key TEXT NOT NULL,
        text TEXT NOT NULL,
        ts INTEGER NOT NULL,
        UNIQUE(kind, key)
    );
    CREATE VIRTUAL TABLE search_fts USING fts5(folded, tokenize = 'unicode61 remove_diacritics 2');
    CREATE TABLE search_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;

INSERT INTO schema_migrations VALUES ('search', '0001', 1780300800000);
INSERT INTO search_docs(id, kind, key, text, ts) VALUES (1, 'turn', '1', 'Jaka będzie pogoda w Toruniu?', 1780300801000);
INSERT INTO search_fts(rowid, folded) VALUES (1, 'jaka bedzie pogoda w toruniu');
