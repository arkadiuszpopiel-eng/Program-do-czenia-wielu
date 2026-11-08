# search-fake

Atrapa modułu `search`: `FakeSearch` (indeks w pamięci; implementuje też `TxIndexer`, ignorując
połączenie), `HashEmbedder` (deterministyczny: trygramy + słowa po `fold_pl` → FNV-1a → wektor 64 wym.,
L2) i `RecordingIndexer` (rejestruje wywołania `TxIndexer`, umie zasymulować błąd). Przechodzi ten sam
`contract_tests::run_all` co `search-impl`.
F7: `FakeSearch` implementuje też `TxSearcher` (zapytanie po etykiecie bazy, FTS AND/OR) — atrapa indeksu dla
`memory-impl` (bazy zakresów) i `contract_tests::tx_search_suite`.
F7-02: `FakeSearch::with_embedder(Arc<dyn Embedder>)` — dokumenty przez `embed`, zapytania przez `embed_query`; błąd
embeddera → dokument bez wektora, zapytanie bez wektora spada do FTS (jak `search-impl`). Eval `memory-impl` z modelem
ONNX (`lib-embed`).
