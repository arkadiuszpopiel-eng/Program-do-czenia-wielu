# search-fake

Atrapa modułu `search`: `FakeSearch` (indeks w pamięci; implementuje też `TxIndexer`, ignorując
połączenie), `HashEmbedder` (deterministyczny: trygramy + słowa po `fold_pl` → FNV-1a → wektor 64 wym.,
L2) i `RecordingIndexer` (rejestruje wywołania `TxIndexer`, umie zasymulować błąd). Przechodzi ten sam
`contract_tests::run_all` co `search-impl`.
