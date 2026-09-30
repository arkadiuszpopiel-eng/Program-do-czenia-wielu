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
