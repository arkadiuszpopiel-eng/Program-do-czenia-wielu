# search-contract

Kontrakt modułu `search` (docs/modules/search/SPEC.md).

- `Search` — `index`/`remove`/`query` (tryby `Fts`, `Vector`, `Hybrid`); `TxIndexer` — indeksowanie w
  transakcji modułu zapisującego dane (`sessions`, `memory`); `Embedder` — lokalny embedder.
- Reguły wspólne dla `-impl`/`-fake`: `authorize` (agentka tylko własna sesja), `fuse_rrf` (RRF, k = 60),
  `sort_hits` (deterministyczny porządek), `make_snippet` (fragment z zakresami podświetleń w znakach —
  **bez HTML**).
- **Wyszukiwanie między sesjami** (`SessionSet::Many/All`) to funkcja UI właściciela (`Caller::Owner`,
  `Ctrl+Shift+F`), otwiera wiele baz — **nigdy narzędzie agentki**.
- `contract_tests` (feature): FTS bez diakrytyków + podświetlenia, wektor/hybryda, rodzaje/limit/determinizm,
  zastąpienie dokumentu, kaskada usunięcia, izolacja agentki (0/1000).
- F7 (addytywnie): `TxSearcher::query_in` + `ConnQuery` — zapytanie w połączeniu modułu-właściciela bazy (np.
  `memory` w bazie zakresu globalnego), osobny tekst FTS i embeddingu, FTS „dowolne słowo” (`match_any`);
  `TxIndexer::compact_in` — zatarcie usuniętych danych indeksu (FTS5 `optimize`); `contract_tests::tx_search_suite`.
- F7-02 (addytywnie): `Embedder::embed_query` (domyślnie = `embed`; E5: `query:` vs `passage:`),
  `TxIndexer::vector_status_in` → `VectorStatus` (`Ready{embedder, missing}` / `Rebuilding{from, to, done, total}`),
  `TxIndexer::reindex_step(&Db, batch)` → `ReindexProgress` (przebudowa wektorów po zmianie embeddera, embedding poza
  blokadą bazy; domyślnie „nic do zrobienia”), zdarzenia `search.reindex.{started,progress,done}`,
  `search.vector.missing`. Produkcyjny embedder: `lib-embed`.
