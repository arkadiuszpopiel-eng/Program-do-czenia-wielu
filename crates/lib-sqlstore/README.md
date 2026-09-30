# lib-sqlstore

Wspólna biblioteka danych (kategoria `lib-*`, `crates/README.md`) — **bez logiki modułu**. Wnioski ze
spike'u (i) i ADR 0008 w jednym miejscu:

- `DbKey` — klucz surowy 32 B z CSPRNG (`getrandom`), zerowany przy `drop` (`zeroize`), `Debug` bez bajtów;
  do SQLCipher trafia jako `PRAGMA key = "x'<64 hex>'"` (bez KDF: otwarcie < 0,2 ms zamiast 142 ms).
- `open_connection` / `Db` — kolejność: rejestracja sqlite-vec (`sqlite3_auto_extension` w `OnceLock`) →
  `cipher_log_level = NONE` → klucz → weryfikacja (`sqlite_master`; zły klucz → `NotDatabaseOrWrongKey`) →
  `journal_mode = WAL`, `synchronous = NORMAL`, `foreign_keys = ON`, `busy_timeout = 5 s`. `Db` = jedno
  połączenie za muteksem (współdzielone przez moduły przez `Arc<Db>`).
- `migrate(conn, namespace, &[(wersja, sql)])` — tabela `schema_migrations(namespace, version)`, każda
  migracja w SAVEPOINT (działa także w otwartej transakcji), wykrywa bazę nowszą od kodu i migracje
  wstawione „w środek”.
- `remove_database` — plik + `-wal`/`-shm`/`-journal`, idempotentnie (crypto-shredding = plik + klucz).
- `fold_pl` / `tokenize` / `search_tokens` / `fts5_match` — normalizacja PL do FTS5: `ł→l`, `Ł→L`,
  diakrytyki przez NFD (`unicode-normalization`), znak→znak (pozycje podświetleń zachowane);
  „zolc” znajduje „żółć”. `fts5_match` cytuje każde słowo (`"zolc"*`) — składnia FTS5 użytkownika nie przechodzi.
- `vector_to_blob` / `blob_to_vector` — format `float[N]` dla `vec0`.

**Wyjątek `unsafe`:** crate ma `unsafe_code = "deny"` (workspace: `forbid`) i jedno `#[allow(unsafe_code)]`
na funkcji rejestrującej sqlite-vec (`src/open.rs`, transmute wskaźnika `sqlite3_vec_init` jak w spike'u i).

Testy: `tests/sqlstore.rs` (szyfrowanie, zły klucz, WAL/FK, sqlite-vec + FTS5 z `fold_pl`, migracje, usuwanie
plików), `tests/fold_props.rs` (property-based: długość, idempotencja, ASCII dla PL, zakresy słów).
