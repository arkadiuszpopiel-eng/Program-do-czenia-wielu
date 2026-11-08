# personas-impl

Implementacja modułu `personas`: `PersonasModule` = `Personas` + `Module` (manifest `module.toml`:
`always`, `inproc`, RAM ≤ 1 MB). Rdzeń (`PersonasState` z kontraktu) trzyma katalog i obsady sesji;
moduł dokłada cykl życia i publikację zdarzeń `personas.*` na magistrali. Zmiana obsady działa
natychmiast, bez restartu sesji; **wymaga uruchomionego modułu** (`NotStarted` przed `start`), bo każda
zmiana musi trafić do dziennika. Odczyty (`cast`, `resolve_addressee`, `system_prompt`) działają zawsze.
Dodatkowo: `set_default_template` (`[personas] default_cast`), `set_prompt_template`, `import` paczki `.alfa`.

Zależności produkcyjne: tylko `*-contract`. Testy: kontrakt współdzielony, manifest, cykl życia,
zdarzenia na `core-bus-fake` (w tym ostrzeżenie „Krytyczka z rolą autorki”), Kreator + eksport/import.
