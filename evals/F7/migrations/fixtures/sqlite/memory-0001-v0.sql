-- Stan bazy sprzed migracji (wykonać na pustej bazie SQLCipher otwartej kluczem testowym).
-- Pamięć v0 (`memory` 0001) w bazie sesji: wpisy JSON bez pól F7.
CREATE TABLE schema_migrations(
    namespace TEXT NOT NULL,
    version TEXT NOT NULL,
    applied_at INTEGER NOT NULL,
    PRIMARY KEY(namespace, version)
) WITHOUT ROWID;
CREATE TABLE memory_entries(
        id TEXT PRIMARY KEY,
        body TEXT NOT NULL,
        approved INTEGER NOT NULL,
        created_at INTEGER NOT NULL
    ) WITHOUT ROWID;

INSERT INTO schema_migrations VALUES ('memory', '0001', 1780300800000);
INSERT INTO memory_entries(id, body, approved, created_at) VALUES ('m-101', '{"id":"m-101","scope":{"scope":"session","id":"s-f1"},"layer":"semantic","text":"Użytkownik ma psa o imieniu Burek.","entities":[],"provenance":{"kind":"user"},"trusted":true,"confidence":1.0,"ttl_secs":null,"created_at":"2026-06-10T09:00:00Z","approved":true}', 1, 1781082000000);
INSERT INTO memory_entries(id, body, approved, created_at) VALUES ('m-102', '{"id":"m-102","scope":{"scope":"session","id":"s-f1"},"layer":"semantic","text":"Rozmowa dotyczyła wyjazdu do Gdańska.","entities":[],"provenance":{"kind":"agent","agent":"alfa"},"trusted":true,"confidence":1.0,"ttl_secs":null,"created_at":"2026-06-10T09:00:00Z","approved":true}', 1, 1781082000000);
