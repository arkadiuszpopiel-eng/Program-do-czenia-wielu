# search-impl

Implementacja modułu `search` w szyfrowanej bazie sesji (ADR 0008):

- `search_docs(kind, key, text, ts)` + `search_fts` (FTS5, kolumna złożona `fold_pl`, tokenizer
  `unicode61 remove_diacritics 2`) + `search_vec_<rodzaj>` (`vec0`, kosinus, `chunk_size=128`) — osobna tabela
  wektorowa na rodzaj dokumentu, żeby kNN z filtrem nie gubił rzadkich wpisów pamięci.
- Zapytania: FTS (bm25, słowa cytowane i z prefiksem), kNN, hybryda RRF; fragmenty `make_snippet`.
- `SqliteSearch` implementuje `Search` (bazy przez `SessionDbProvider`), `TxIndexer` (z `compact_in` = FTS5
  `optimize`) i `TxSearcher` (zapytanie w połączeniu wywołującego, FTS „dowolne słowo” — recall pamięci F7).
- Identyfikator embeddera zapisany w bazie; zmiana → `EmbedderMismatch` (reindeksacja — później).
- `Module` + `module.toml`; zdarzenie `search.query` tylko z licznikami.

Testy: kontrakt, kaskada w surowych tabelach, testy szpiegowskie, wycofanie z transakcją wywołującego,
zmiana embeddera, zdarzenia, budżet (FTS na 1000 turach < 20 ms), `tx_search_suite` na szyfrowanej bazie,
zatarcie słów usuniętego dokumentu w tabelach wewnętrznych FTS5.
