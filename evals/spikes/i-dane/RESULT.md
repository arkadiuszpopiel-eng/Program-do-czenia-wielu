# Spike (i) — dane: SQLCipher + sqlite-vec + FTS5 w jednej bazie (wynik)

| Pole | Wartość |
|---|---|
| Data | 2026-09-30 |
| Środowisko | Linux x86-64 (kontener, 4 rdzenie, 15 GB RAM), Rust 1.94.1, cargo 1.94.1 |
| Kod | `crates/spike-data` (lib + bin pomiarowy + `tests/spike.rs`) |
| Uruchomienie | `cargo test -p spike-data` (dowody), `cargo run --release -p spike-data` (pomiary) |
| Werdykt | **DZIAŁA** — wszystkie trzy elementy w jednym pliku, bez ładowania DLL; ADR 0008 do potwierdzenia z uwagami niżej |

## Co sprawdzono (wszystko zielone)

1. Otwarcie bazy z kluczem (`PRAGMA key`), `PRAGMA journal_mode = WAL`.
2. Schemat w jednym pliku: `messages(id, session_id, text)`, `messages_vec USING vec0(embedding float[8])`,
   `messages_fts USING fts5(text, content='messages', tokenize='unicode61')`.
3. 100 rekordów w trzech tabelach w jednej transakcji.
4. kNN: `SELECT rowid, distance FROM messages_vec WHERE embedding MATCH ?1 AND k = 5 ORDER BY distance` —
   top-1 trafia we właściwy rekord w 100/100 zapytaniach, dystans 0 dla własnego wektora, wyniki posortowane.
5. FTS5 po polsku: „żółć” (10/100), „sesja” (100/100), „jaźń”, fraza `"kolor żółć"`, brak fałszywych trafień.
6. Zamknięcie i **otwarcie bez klucza → `file is not a database`**; **zły klucz → ten sam błąd**;
   nagłówek pliku ≠ `SQLite format 3\0`, treść wiadomości nie występuje jawnie w bajtach pliku.
7. Ponowne otwarcie z dobrym kluczem: dane, wektory i FTS kompletne.
8. Crypto-shredding per sesja: dwie bazy (dwa pliki, dwa klucze); klucz A nie otwiera B; usunięcie pliku A
   (+ `-wal`, `-shm`) = brak danych (ponowne otwarcie tworzy pustą bazę), B nietknięta.
9. `sqlite_compileoption_used('ENABLE_FTS5') = 1` w bundlu SQLCipher.

## Wersje i feature'y

| Składnik | Wersja | Uwagi |
|---|---|---|
| `rusqlite` | 0.40.2 | `default-features = false`, features `cache` + `bundled-sqlcipher-vendored-openssl` (bez `bundled-full`; domyślne wyłączone, bo ciągną `sqlite-wasm-rs` do `Cargo.lock`) |
| `libsqlite3-sys` | 0.38.2 | bundle SQLCipher kompilowany z `SQLITE_ENABLE_FTS5`, `SQLITE_ENABLE_JSON1`, `SQLITE_HAS_CODEC`, `SQLITE_TEMP_STORE=2` |
| SQLite (w bundlu SQLCipher) | 3.51.3 | zwykły bundle rusqlite ma 3.53.2 — SQLCipher jest ~2 wydania za |
| SQLCipher | 4.14.0 community | `PRAGMA cipher_version`; AES-256-CBC + HMAC-SHA512, PBKDF2 256 000 iteracji (domyślnie) |
| OpenSSL (vendored) | 3.6.3 (`openssl-src` 300.6.1+3.6.3, `openssl-sys` 0.9.117) | kompilowany ze źródeł przy buildzie |
| `sqlite-vec` | 0.1.9 (`vec_version()` = v0.1.9) | crate kompiluje `sqlite-vec.c` z `SQLITE_CORE` i linkuje statycznie; rejestracja przez `rusqlite::ffi::sqlite3_auto_extension` (bez `load_extension`, bez DLL) |

Pinowanie: `[workspace.dependencies]` w root `Cargo.toml` (`rusqlite`, `sqlite-vec`), crate używa `workspace = true`.

## Pomiary (100 rekordów, embedding 8 wymiarów, 1 wątek)

| Pomiar | debug | release |
|---|---|---|
| 100 wstawień (3 tabele, 1 transakcja) | 3,7 ms | 1,8 ms |
| 100 zapytań kNN, k = 5 | 6,7 ms (67 µs/zap.) | 2,8 ms (28 µs/zap.) |
| 100 zapytań FTS5 „żółć” | 6,7 ms | 2,5 ms |
| Rozmiar pliku po zamknięciu (WAL scalony) | 102 400 B (25 stron × 4 KiB) | j.w. |
| Otwarcie z hasłem (KDF PBKDF2 256k) | **142 ms** | **142 ms** |
| Otwarcie z kluczem surowym `x'…'` (bez KDF) | 124 µs | 67 µs |
| Otwarcie bez klucza (błąd) | 0,2 ms | 0,1 ms |

Czas builda (czysty build nowych zależności: openssl-src, libsqlite3-sys, sqlite-vec, rusqlite; 4 rdzenie):
**1 min 19 s** wall (3 min 28 s CPU) w `dev`; `release` (lto = fat, codegen-units = 1): **2 min 07 s**.
Binarka release ze `strip`: **7,1 MB** (większość to libcrypto OpenSSL). `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check`, `cargo test -p spike-data`, `cargo deny check` — zielone.

## Licencje / cargo-deny

Nowe crate'y: rusqlite, libsqlite3-sys, openssl-sys (MIT); sqlite-vec, openssl-src, fallible-*, vcpkg, hashlink, cc,
pkg-config, smallvec, bitflags (MIT OR Apache-2.0); foldhash (Zlib). Wszystko na liście `deny.toml` — **bez zmian
w `deny.toml`**. Kod C w bundlach: SQLCipher — BSD-3-Clause (Zetetic), SQLite — public domain, OpenSSL — Apache-2.0
(wewnątrz crate'a `openssl-src`). `bans multiple-versions`: bez nowych duplikatów (ostrzeżenie `syn` 2/3 istniało wcześniej).

## Problemy i obserwacje

1. **KDF 142 ms na otwarcie** przy haśle tekstowym. Klucz sesji i tak leży w Credential Manager/DPAPI, więc ma być
   **losowym kluczem 32 B podawanym jako surowy** (`PRAGMA key = "x'<64 hex>'"`) → otwarcie < 0,2 ms. Alternatywa:
   `PRAGMA cipher_kdf_iter` w dół (gorsza ochrona hasła — niepotrzebna przy kluczu losowym).
2. **SQLCipher loguje na stderr** przy złym kluczu (`ERROR CORE sqlcipher_page_cipher: hmac check failed`). W produkcji:
   `PRAGMA cipher_log_level = NONE` (albo `cipher_log_source`) przed `PRAGMA key`, żeby nie zaśmiecać logów Alfy.
3. **Polskie „ł” a FTS5 `unicode61` (`remove_diacritics=1`)**: „żółć” ↔ „zółc” pasują (ż→z, ć→c), ale **„zolc” nie**
   (ł nie ma dekompozycji Unicode, więc nie jest „diakrytykiem”); „gęś” ↔ „ges” pasuje. Dla wyszukiwania bez
   diakrytyków trzeba własnej normalizacji (ł→l) przed indeksowaniem i w zapytaniu, albo tokenizera `trigram`.
   Do rozstrzygnięcia w module `search` (F7, recall@5 na PL).
4. **`unsafe` jest konieczne raz** (transmute wskaźnika `sqlite3_vec_init` + `sqlite3_auto_extension`), a workspace
   ma `unsafe_code = "forbid"`. Crate spike'u definiuje własne `[lints]` (kopia workspace z `unsafe_code = "allow"`).
   Docelowy crate danych będzie potrzebował tego samego wyjątku (ADR/zapis w `crates/README.md`), jedno wywołanie
   w `Once`, przed pierwszym połączeniem.
5. **Windows nie był tu testowany** (spike biegł na Linuksie). `openssl-src` na MSVC wymaga **Perla** (Strawberry Perl)
   i opcjonalnie NASM; czas builda OpenSSL na Windows będzie dłuższy (kilka minut, jednorazowo, cache w `target/`).
   Do potwierdzenia na runnerze Windows w CI (F0-14). Wariant bez vendoringu (`bundled-sqlcipher` + `OPENSSL_DIR`)
   przenosi zależność na vcpkg — nie polecam.
6. **WAL**: pliki `-wal`/`-shm` są szyfrowane przez SQLCipher (strony), ale `forget` musi usuwać je razem z bazą
   (`remove_database` w spike'u to robi). `SQLITE_TEMP_STORE=2` → pliki tymczasowe w pamięci (bez jawnych zrzutów na dysk).
7. `sqlite-vec` 0.1.x to **brute-force kNN** (bez indeksu ANN): 28 µs/zapytanie przy 100 wektorach × 8 wym.;
   przy 384–1024 wym. i dziesiątkach tysięcy wpisów per sesja trzeba zmierzyć (F7). Rozmiar per sesja jest mały,
   więc to raczej wystarczy; awaryjnie `vec0` obsługuje partycjonowanie i `int8`/`bit` kwantyzację.
8. SQLite w bundlu SQLCipher (3.51.3) jest starszy od zwykłego bundla (3.53.2) — bez wpływu na funkcje, ale
   podnoszenie wersji SQLite idzie przez wydania SQLCipher/`libsqlite3-sys`.
9. Rozmiar binarki +~5 MB (OpenSSL). Do zaakceptowania (§3.4 „lekki”: to nadal jeden proces, zero usług).

## Rekomendacja do ADR 0008

**Potwierdzić** decyzję: SQLite (WAL) + SQLCipher + sqlite-vec + FTS5 w **jednej szyfrowanej bazie per sesja**;
sqlite-vec linkowany statycznie i rejestrowany przez `sqlite3_auto_extension` (nie `load_extension`). Uzupełnić ADR o:
klucze surowe 32 B z Credential Manager (bez KDF), `cipher_log_level = NONE`, usuwanie `-wal`/`-shm` w kaskadzie
`forget`, normalizację „ł” dla FTS (moduł `search`), jawny wyjątek od `unsafe_code = forbid` dla crate'a danych oraz
wymaganie Perla na runnerach Windows. Wariant zapasowy z sekcji „Jak cofnąć” (szyfrowanie na poziomie aplikacji /
LanceDB) **nie jest potrzebny**.
