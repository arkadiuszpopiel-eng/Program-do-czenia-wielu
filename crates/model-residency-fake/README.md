# model-residency-fake

Atrapa `Residency` (tylko `dev-dependency` innych modułów: `voice-*`, `providers-local`, `search`).
Te same reguły co `-impl` (maszyna stanów `LeaseTable` z kontraktu), ręczny zegar (`clock().advance_ms`),
`fail_next(ResidencyError)`, zapis wywołań (`calls()`) i zdarzeń `residency.*` (`events()`),
słuchacze właścicieli. `FakeResidency::baseline()` = budżet baseline (8 GB − 768 MB pulpitu).
