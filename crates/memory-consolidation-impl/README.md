# memory-consolidation-impl

Adaptery Strażniczki pamięci + moduł zadania tła (docs/modules/memory-consolidation/SPEC.md):
`LlmConsolidator` (`ModelProvider`; prompt PL, epizody jako dane JSON, odpowiedź wyłącznie JSON, sesje prywatne z
tagiem `Private`), `CostMeterBudget` (budżet tła `cost-meter`, rejestracja zużycia z `background = true`),
`DeviceHost` (`DeviceProfile` + `IdleSource` + `LocalClock`), `ConsolidationModule` (`module.toml`, harmonogram co
15 min, `run_now`, ostatni raport). Testy: adapter modelu na atrapie dostawcy (także przebieg end-to-end), budżet na
atrapie licznika, ACCEPTANCE F7-05 na atrapie `device-profile` (20 scenariuszy, 0 startów), cykl życia modułu.
