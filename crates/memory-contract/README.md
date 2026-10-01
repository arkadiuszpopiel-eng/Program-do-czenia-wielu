# memory-contract

Kontrakt pamięci (docs/modules/memory/SPEC.md, PLAN §10, §8.7).

- **v0** `Memory` (`remember`, `recall`, `get`, `list`, `approve`, `forget`, `promote`) — fasada zakresu sesji;
  reguły `validate_new`, `recall_sessions`, `is_expired`, `check_promotion`.
- **F7** `MemoryService` — cztery warstwy, zakresy `Session | Project | Global | Agent` z uprawnieniami
  (`Accessor`, `AgentAccess`, `ScopeGrant`, `authorize`), proweniencja (`Provenance`, `Origin`, `EntryRef`), wersje
  (`supersedes`, `Supersession`, `EntryState`), prywatność (`PrivacyOracle`), recall z rerankingiem (`Reranker`,
  `HeuristicReranker`, rdzenie PL w `rerank`), Inspektor (`InspectorQuery`, `Explanation`, `EntryEdit`), zestaw roboczy,
  `forget` kaskadowo (`ForgetTarget`, `plan_cascade`, `CascadeReport`), dziennik z cofaniem (`ChangeSet`, `ChangeOp`,
  `JournalRecord`), eksport/import (`export`, `ImportPolicy`).
- **Silnik** `MemoryEngine<B: MemoryBackend>` z portami (`EnginePorts`: zegar, identyfikatory, prywatność, reranker,
  zdarzenia) — wspólny dla `-impl` (SQLCipher) i `-fake` (mapy); implementuje `MemoryService` i `Memory`.
- `contract_tests` (v0) i `contract_tests_f7` (feature `contract-tests`): warstwy i zakresy, uprawnienia agentek,
  prywatność, F7-04 (50 prób awansu niezaufanego), wersje i sprzeczności, przypięcie i zestaw roboczy, TTL, Inspektor,
  „dlaczego”, eksport/import, kaskada (wpis, sesja, źródło, tura, zakres; F7-03: 50 usunięć zweryfikowanych), dziennik
  i cofanie, atomowość, `Resolve`, testy szpiegowskie (F7-01: 3+ sesje z prywatną, 1000 zapytań, 0 przecieków),
  zdarzenia bez treści.
