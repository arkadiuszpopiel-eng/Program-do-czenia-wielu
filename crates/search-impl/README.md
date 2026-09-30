# search-impl

Implementacja modułu `search` w szyfrowanej bazie sesji (ADR 0008):

- `search_docs(kind, key, text, ts)` + `search_fts` (FTS5, kolumna złożona `fold_pl`, tokenizer
  `unicode61 remove_diacritics 2`) + `search_vec_<rodzaj>` (`vec0`, kosinus, `chunk_size=128`) — osobna tabela
  wektorowa na rodzaj dokumentu, żeby kNN z filtrem nie gubił rzadkich wpisów pamięci.
- Zapytania: FTS (bm25, słowa cytowane i z prefiksem), kNN, hybryda RRF; fragmenty `make_snippet`.
- `SqliteSearch` implementuje `Search` (bazy przez `SessionDbProvider`) i `TxIndexer`.
- Identyfikator embeddera zapisany w bazie; zmiana → `EmbedderMismatch` (reindeksacja — później).
- `Module` + `module.toml`; zdarzenie `search.query` tylko z licznikami.

Testy: kontrakt, kaskada w surowych tabelach, testy szpiegowskie, wycofanie z transakcją wywołującego,
zmiana embeddera, zdarzenia, budżet (FTS na 1000 turach < 20 ms).
