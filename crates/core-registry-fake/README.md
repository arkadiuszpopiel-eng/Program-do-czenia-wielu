# core-registry-fake

Deterministyczna atrapa rejestru modułów (docs/modules/core-registry/SPEC.md, „Fake”) do testów
innych modułów (tylko `dev-dependencies`). `FakeRegistry` implementuje `Registry` z kontraktu
i przechodzi ten sam test kontraktowy co `core-registry-impl` (graf z `DependencyGraph`, te same
reguły cyklu życia). Różnice: czas bezczynności jest wirtualny (`advance`), brak limitu crash-loop,
a do testów dochodzą: `register_manifest` (moduł-wydmuszka z fixture'a), `set_state` (symulacja
`Failed`/`Degraded`), `fail_next_start` (jednorazowa awaria startu) i `calls()` (dziennik wywołań).
