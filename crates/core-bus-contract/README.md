# core-bus-contract

Kontrakt magistrali zdarzeń jądra (docs/PLAN.md §13, docs/modules/core-bus/SPEC.md).
Zawiera: typ `Event` (id, ts, sesja, agentka, przebieg, span, rodzaj, poziom, payload, koszt, `prev_hash`),
`EventKind`, `Level` (Trace…Audit), `EventFilter`, asynchroniczny trait `EventBus`
(`publish` nie blokuje; `subscribe(filter)` zwraca strumień) oraz JSON Schema zdarzenia
(`event_schema()`, `EVENT_SCHEMA_VERSION = 1`) zapisany w `packages/schemas/event.v1.json`.
Test `schema_snapshot` porównuje wygenerowany schemat z plikiem; po zmianie typów uruchom
`UPDATE_SCHEMAS=1 cargo test -p core-bus-contract`, a CI pilnuje `git diff --exit-code`.
Funkcja `contract_tests::run_all` (feature `contract-tests`) uruchamia ten sam zestaw przypadków
przeciw `core-bus-impl` i `core-bus-fake`.
