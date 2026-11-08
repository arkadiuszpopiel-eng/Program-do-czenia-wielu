# core-registry-impl

Rejestr modułów jądra (docs/modules/core-registry/SPEC.md). `ModuleRegistry` implementuje trait
`Registry` z `core-registry-contract`: `register` (manifest + `Box<dyn Module>`), `start_order`
(graf kontraktów `nazwa-contract@major`: braki, konflikty, cykle → `RegistryError`, nigdy panika),
`boot` (moduły `always` + zależności), `acquire` (pierwsze użycie kontraktu startuje dostawcę `lazy`),
`activate` (`on-demand`), `deactivate` (najpierw zależne), `set_enabled`, `health`, `unload_idle`
(bezczynność > limit; zależne przed zależnościami; `always` nigdy), `shutdown`.
Crash-loop: po `crash_loop_limit` (3) nieudanych startach rejestr odmawia (`CrashLoop`) do
wyłączenia/włączenia modułu. Zegar wstrzykiwany (`with_clock`) — w testach wirtualny;
`spawn_idle_reaper` woła `unload_idle` okresowo. Zdarzenia: `registry.module.state_changed`,
`registry.module.health`, `registry.resolve_failed`. Manifest własny: `module.toml` (test).
