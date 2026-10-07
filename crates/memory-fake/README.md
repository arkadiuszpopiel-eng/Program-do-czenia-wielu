# memory-fake

Atrapy pamięci (docs/modules/memory/SPEC.md, „Fake”):

- `FakeMemory` (v0) — wpisy w RAM, `recall` przez dopasowanie słów (bez diakrytyków, prefiksy), wirtualny zegar,
  identyfikatory `mem-0001`…, licznik kaskad `forget`.
- `FakeMemoryService` (F7) = `memory_contract::MemoryEngine<FakeBackend>` — **ta sama logika co `memory-impl`**
  (uprawnienia, prywatność, wersje, kaskada, dziennik, recall z rerankingiem); `FakeBackend` trzyma wpisy, dziennik i
  notatki eksportów w mapach, wyszukuje leksykalnie po rdzeniach PL, zgłasza crypto-shredding zakresów własnych;
  `service()` (porty deterministyczne) / `service_with(ports)`.

Testy: `contract_tests::run_all` (v0) na `FakeMemory` i na silniku, `contract_tests_f7::run_all` (19 przypadków).
