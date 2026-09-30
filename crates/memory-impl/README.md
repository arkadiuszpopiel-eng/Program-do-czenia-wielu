# memory-impl

Pamięć v0 w bazie sesji: `memory_entries(id, body, approved, created_at)` + dokumenty `DocKind::Memory`
indeksu `search` zapisywane w tej samej transakcji (`TxIndexer`). `recall` = hybryda FTS + wektor przez
`Search` wywoływany jako agentka tej sesji (najmniejsze uprawnienia), potem filtr: zatwierdzone, niewygasłe.
`forget` usuwa wpis, FTS i wektor w jednej transakcji i zwraca `ForgetReport`. `Module` + `module.toml`,
zdarzenia `memory.*` bez treści. Testy: kontrakt (z `FakeSearch` jako indeksem), szyfrowanie wpisów,
kaskada w tabeli, zdarzenia.
