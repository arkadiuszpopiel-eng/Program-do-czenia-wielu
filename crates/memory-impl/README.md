# memory-impl

Implementacja pamięci (docs/modules/memory/SPEC.md).

- **v0** `SqliteMemory`: `memory_entries` w szyfrowanej bazie sesji + dokumenty `DocKind::Memory` indeksu `search` w tej
  samej transakcji; `recall` przez `Search` jako agentka sesji; pomija wersje zastąpione F7.
- **F7** `MemoryModule` / `SqliteMemoryService` = silnik `memory-contract` nad `SqliteBackend`:
  zakres sesji w bazie sesji, projekt/agentka/globalna w osobnych bazach SQLCipher z kluczem w sejfie
  (`VaultScopeDbs`, crypto-shredding zakresu); indeks i zapytania przez `TxIndexer`/`TxSearcher` w bazie zakresu;
  dziennik `memory_journal`, notatki eksportów `memory_exports` (migracja `0002`, wspólna z v0); po usunięciach
  `secure_delete`, kompakcja FTS i checkpoint WAL. `UuidIds`, `CatalogPrivacy` (prywatność z katalogu sesji),
  zdarzenia `memory.*` bez treści.
- **`transfer`**: `MemoryDocuments` — `DocumentStore` kategorii `memory` (NDJSON per zakres, bez sesji prywatnych,
  walidacja całego dokumentu przed zapisem).
- **Recall@5** (`eval`): format NDJSON (`corpus`, `queries`), syntetyczny zestaw PL (249 zapytań) z
  `eval::synthetic_set`, runner recall@k / trafienie@1 / MRR; pliki w `evals/F7/recall/`.

Testy: kontrakty v0 i F7 na prawdziwych bazach (indeks: `FakeSearch`), szyfrowanie i crypto-shredding zakresu
globalnego, zgodność v0/F7 na jednej bazie, zatarcie surowych tabel, property `forget` (każda droga odczytu: recall,
Inspektor, FTS/wektor/hybryda indeksu, surowe tabele), round-trip `.alfa` przez silnik `transfer`, recall@5.
