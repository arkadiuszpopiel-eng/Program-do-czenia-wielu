-- Stan bazy sprzed migracji (wykonać na pustej bazie SQLCipher otwartej kluczem testowym).
-- Uszkodzony dziennik migracji: `memory` 0002 zapisana bez 0001 (np. ręczna naprawa bazy).
CREATE TABLE schema_migrations(
    namespace TEXT NOT NULL,
    version TEXT NOT NULL,
    applied_at INTEGER NOT NULL,
    PRIMARY KEY(namespace, version)
) WITHOUT ROWID;
INSERT INTO schema_migrations VALUES ('memory', '0002', 1780300800000);
