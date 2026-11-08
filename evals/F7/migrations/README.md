# evals/F7/migrations — migracje schematów (ACCEPTANCE F7-08)

**Status: szkic — wymaga akceptacji człowieka, potem zamrożenie hashem.** Autorka: model-recenzentka (fala 4).

Kryterium **F7-08**: wszystkie wersje schematów od F1 migrują bez błędu (upcastery), a wersje, których nie da się
przyjąć, są odrzucane czytelnie i **bez zapisu** (PLAN §15.1, `docs/formats/alfa-package.md` §6–§7,
`docs/modules/transfer/SPEC.md`).

## Zawartość

| Ścieżka                    | Opis                                                                                                                                                       |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `scenarios.json`           | 24 scenariusze: wejście, oczekiwany wynik, testy już pokrywające zachowanie (`covered_by`), uwagi                                                          |
| `fixtures/alfa-v0/`        | rozpakowana paczka w formacie v0 (`format: 0`, nagłówek i tury v0, 8 tur z rozgałęzieniem)                                                                 |
| `fixtures/alfa-v1/`        | rozpakowany eksport F1 (`schema_version` 1.0.0): sesja z gałęzią, usłyszanym prefiksem, turą ukrytą, użyciem modelu, szkicem; manifest z poprawnymi sumami |
| `fixtures/memory/*.ndjson` | linie dokumentów pamięci (`memory/<zakres>.ndjson`): v0 bez pól F7, F7, przypadki odrzucane                                                                |
| `fixtures/sqlite/*.sql`    | stany baz sprzed migracji (sesje 0001, pamięć 0001 z wpisami v0, indeks 0001, baza z nowszej wersji, uszkodzony dziennik)                                  |

Pokrycie: **paczka `.alfa`** — manifest (m-01, m-04…m-06, m-10, m-12), sesja i tury (m-01…m-03, m-07…m-09, m-11),
sekrety starszych wersji (m-12, m-13); **pamięć** — dokumenty v0 → F7 i reguły importu (m-14…m-19);
**bazy SQLite** — sesje, pamięć, indeks wyszukiwania (m-20…m-24). Wersje schematów w kodzie dziś: paczka 1.0.0 (+ v0
szkic), rekordy sesji v1 (+ v0), `sessions` 0001, `sessions.index` 0001, `memory` 0001–0002, `search` 0001–0002.

## Format scenariusza

`id` (`m-NN-opis`), `entity` (`alfa.package`, `alfa.manifest`, `alfa.session`, `alfa.turns`, `memory.document`,
`sqlite.sessions`, `sqlite.memory`, `sqlite.search`), `from`/`to` (wersje), `title`, `input`, `expected`,
`covered_by`, opcjonalnie `notes`.

- **`input`**: `package` (katalog rozpakowanej paczki — runner pakuje go do ZIP; plik `manifest.alfa.json` trafia
  do paczki jako `manifest.json`, pierwszy i nieskompresowany — w repo ma inną nazwę, bo katalog zestawów `evals`
  traktuje każdy `manifest.json` jako manifest zestawu), `manifest_from` + `set`/`append_file` (zmodyfikowana kopia manifestu), `add_entry` (dodatkowy
  plik w paczce — runner przelicza manifest), `turns_inline`/`header_inline`/`cases` (rekordy w treści
  scenariusza — celowo niepoprawne JSON-y nie mogą być plikami `*.json`, bo CI parsuje każdy taki plik),
  `session_from` + `as_id`, `document` + `lines` (dokument pamięci), `sql` + `namespace` (skrypt wykonywany na
  pustej bazie SQLCipher otwartej kluczem testowym, potem `lib_sqlstore::migrate` z migracjami modułu),
  `workdir_root` (`{SESJE}` = korzeń katalogów roboczych na maszynie importu).
- **`expected`** — dopasowanie **podzbioru** (pola nieobecne w oczekiwaniu nie są sprawdzane): `outcome`
  (`ok`/`error`), `error` (`TransferError` w postaci serde, np. `{"error": "newer_schema", "found": "1.1.0"}`;
  dla SQLite — wariant `StoreError`), `upcast` (kroki raportu dry-run `report.migrations`, w kolejności),
  `files_intact` (SHA-256 i rozmiary każdego wpisu), `sessions` (tury: `id`, `parent`, `branch`, `role`,
  `author`, `heard`, `hidden`, `usage`; `roundtrip_equal` — eksport → import daje tę samą sesję;
  `turns_bytes_identical` — ponowny eksport bajt w bajt), `entries` (wpisy pamięci po imporcie),
  `rejected`/`document_rejected`/`written`, `applied` (zastosowane migracje SQLite), `schema_unchanged`
  (liczba obiektów `sqlite_master` przed = po), `reason_contains`.

## Weryfikacja

Oczekiwane wyniki sprawdziłam jednorazowym programem poza repozytorium na kodzie z gałęzi
`ccr-af4b63c6-3fyzaj` (po `728698a`): `transfer-contract` (`migrate`, `portable`), `memory-contract`
(`export::decode_ndjson`, `check_imported`), `lib_sqlstore::migrate` z migracjami `sessions-impl`, `memory-impl`,
`search-impl` — **23/24 zgodne**; m-13 i część m-12 (odmowa w planie importu) pokrywają istniejące testy
`transfer-impl/tests/secrets.rs`. Fixture'y `alfa-v1` i stany SQL wygenerowałam z tych samych typów i stałych co
aplikacja, więc sumy w manifeście i SQL migracji są zgodne z kodem.

## Runner (częściowo zbudowany)

Fala 5: scenariusze `alfa.manifest` (m-04, m-05, m-06, m-10) — `transfer-contract/tests/f7_manifest.rs`;
`sqlite.sessions` (m-20, m-23) — `sessions-impl/tests/f7_migrations.rs`. Reszta do zbudowania.

Proponowane miejsce: testy czytające `scenarios.json` przez `include_str!` (wzór `evals/F8`) — paczki i
dokumenty pamięci w `transfer-impl/tests/migrations.rs` (silnik importu z `MemoryDocuments`, dry-run + import +
rollback), bazy w `app-core/tests/migrations.rs` albo w testach `sessions-impl`/`memory-impl`/`search-impl`.
Każdy scenariusz `error` sprawdza dodatkowo, że import nic nie zapisał (snapshot i liczba elementów bez zmian).

## Zasada utrzymania (ACCEPTANCE F7-08 „od F1”)

Każda zmiana schematu (nowa wersja paczki, rekordu sesji, wpisu pamięci albo migracja SQLite) **dokłada** tu
fixture poprzedniej wersji (`alfa-v<N>`, `sqlite/<moduł>-<wersja>.sql`) i scenariusz migracji do bieżącej —
istniejących fixture'ów się nie zmienia (zamrożone hashem).

## Do decyzji człowieka

1. **m-23 — powrót do starszej wersji po migracji.** *Fala 5: wdrożone w `lib_sqlstore::migrate` — baza z nowszej
   wersji otwiera się bez zmian w trybie tylko do odczytu, a migracje oznaczone jako addytywne (`schema_compat`)
   pozwalają pracować normalnie; scenariusz m-23 zmieniony na `outcome: ok` (`read_only`, `write_blocked`,
   `schema_unchanged`). Uzasadnienie i propozycja dla aktualizatora: `docs/modules/sessions/SPEC.md` („Fala 5”) —
   do akceptacji człowieka razem z zestawem.* Opis pierwotny: Starsza wersja aplikacji (aktualizator, wersje obok siebie)
   dostaje `UnknownMigration` na bazie zmigrowanej przez nowszą (dziś realne: `search` 0002, uwaga w SPEC
   `search`). Fail-closed, ale sesje są nieczytelne do ponownej aktualizacji, a ADR 0007 wymaga, by rollback nie
   niszczył danych. Opcje: migracje wyłącznie addytywne + lista wersji zgodnych wstecz w `schema_migrations`, kopia
   bazy przed migracją, albo blokada rollbacku przez aktualizator przy niezgodnym schemacie.
2. **m-06** — upcaster dla starszego „major” i komunikat (dziś „nowsza wersja”) — przy pierwszej zmianie major.
   *Fala 5: bramka wersji `check_schema_version` przed odczytem struktury; starsze major bez upcastera →
   `older_schema` (`found`, `oldest`), bez rady „zaktualizuj”; scenariusz m-06 zmieniony na `older_schema`.*
3. Zamrożenie: `evals/F7/migrations/MANIFEST.json` z SHA-256 wszystkich plików tego katalogu.
