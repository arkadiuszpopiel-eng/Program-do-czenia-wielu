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
  migracja w SAVEPOINT (działa także w otwartej transakcji), wykrywa migracje wstawione „w środek” i luki
  w historii. **Baza z nowszej wersji programu** (rollback, ADR 0007; fala 5, m-23): nieznane migracje nowsze od
  każdej znanej → `Ok` bez zmian (`MigrationReport::newer`), połączenie w `PRAGMA query_only`
  (`read_only`), chyba że nowsza wersja oznaczyła je jako addytywne — `migrate_with(.., additive)` zapisuje je
  w `schema_compat`. Oczekująca migracja na połączeniu tylko do odczytu → `StoreError::ReadOnly`. Zasady
  migracji addytywnych: `docs/modules/sessions/SPEC.md` („Fala 5”).
- `remove_database` — plik + `-wal`/`-shm`/`-journal`, idempotentnie (crypto-shredding = plik + klucz).
- `fold_pl` / `tokenize` / `search_tokens` / `fts5_match` — normalizacja PL do FTS5: `ł→l`, `Ł→L`,
  diakrytyki przez NFD (`unicode-normalization`), znak→znak (pozycje podświetleń zachowane);
  „zolc” znajduje „żółć”. `fts5_match` cytuje każde słowo (`"zolc"*`) — składnia FTS5 użytkownika nie przechodzi.
- `vector_to_blob` / `blob_to_vector` — format `float[N]` dla `vec0`.

**Wyjątek `unsafe`:** crate ma `unsafe_code = "deny"` (workspace: `forbid`) i jedno `#[allow(unsafe_code)]`
na funkcji rejestrującej sqlite-vec (`src/open.rs`, transmute wskaźnika `sqlite3_vec_init` jak w spike'u i).

Testy: `tests/sqlstore.rs` (szyfrowanie, zły klucz, WAL/FK, sqlite-vec + FTS5 z `fold_pl`, migracje, usuwanie
plików), `tests/rollback.rs` (baza z nowszej wersji: tylko odczyt / addytywne / luka), `tests/fold_props.rs` (property-based: długość, idempotencja, ASCII dla PL, zakresy słów).
