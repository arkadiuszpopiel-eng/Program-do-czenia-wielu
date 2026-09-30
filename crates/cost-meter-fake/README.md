# cost-meter-fake

Atrapy licznika kosztów:

- `FakeCostMeter` — `CostMeter` w pamięci ze stałym kursem i dniem (`set_day`), wymuszanym werdyktem
  `check_budget` (`force_decision`) i rejestrem zapytań (`checks`) — dla testów `router`/`agent-runtime`;
  bez wymuszenia liczy decyzję tą samą funkcją `evaluate` co `-impl`;
- `MemoryLedgerStore` — dziennik w pamięci (`fail_next_append` do ścieżek błędów);
- `FakeFxSource` — kurs ze skryptu (`fixed`, `offline`, kolejka `push`), liczy wywołania;
- `FixedClock` — sterowany dzień (`set_day`, `advance_days`).

Przechodzi ten sam `contract_tests::run_all` co `cost-meter-impl`. Tylko jako `dev-dependency`.
