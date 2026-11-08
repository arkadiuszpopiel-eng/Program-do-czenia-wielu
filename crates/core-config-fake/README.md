# core-config-fake

Atrapa konfiguracji (docs/modules/core-config/SPEC.md, „Fake”) do testów innych modułów (tylko
`dev-dependencies`). `FakeConfigStore` (`ConfigStore`) trzyma warstwy w pamięci z tymi samymi
regułami co `core-config-impl`: Default < Shared < Machine(bieżąca), nadpisania sesji/agentek,
`kernel.*` tylko przez Broker (wspólne `authorize` z kontraktu), Default i cudza maszyna tylko do
odczytu, bez `null`/obiektów. Dodatki: `with_defaults`, `simulate_file_change(layer, toml)`
(fixture TOML jako „edycja pliku” → `ConfigChange` dla obserwatorów), `history()` w pamięci,
`fail_next_set`. Bez plików, git i walidacji JSON Schema (tę testuje `-impl`).
