# sessions-fake

Atrapa modułu `sessions` (tylko `dev-dependencies` innych modułów):

- `FakeSessions` — sesje w pamięci, identyfikatory `sess-0001`…, wirtualny zegar (+1 s/operację); przechodzi
  ten sam `contract_tests::run_all` co `sessions-impl`.
- `MemoryKeyVault` — sejf kluczy w pamięci (z przełącznikiem „niedostępny”).
- `TempDbProvider` — `SessionDbProvider` z **prawdziwymi** bazami SQLCipher w katalogu tymczasowym — dla testów
  `search`/`memory`/`artifacts` bez zależności od `sessions-impl`.
- `fixtures` — `barge_in_conversation` (gałęzie + usłyszany prefiks przybliżony), `linear_conversation(n)`.
