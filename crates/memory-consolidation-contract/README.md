# memory-consolidation-contract

Kontrakt Strażniczki pamięci (docs/modules/memory-consolidation/SPEC.md): `Guardian` (przebieg na
`MemoryService` jako `Accessor::Guardian`), polityka `may_start` (bateria, tryb gry, okno, bezczynność), porty
`Consolidator` (LLM), `BackgroundBudget` (`cost-meter`), `HostConditions`; reguły deterministyczne (`rules`: retencja,
duplikaty, sprzeczności, walidacja propozycji modelu); `undo_run`. Testy (`tests/`) na `memory-fake` i atrapach
portów: reguły i cofnięcie przebiegu, ekstrakcja (tryb „ask”, umiejętności, walidacja źródeł), treść niezaufana i
prywatna poza modelem chmurowym i pamięcią globalną (F7-04), budżet, F7-05, przerwanie między zakresami.
