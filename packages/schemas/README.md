# packages/schemas/

JSON Schema **generowane z Rust** (schemars) — nie edytuj ręcznie. Źródło prawdy to typy w crate'ach
`*-contract`; plik w tym katalogu jest snapshotem, który CI porównuje przez `git diff --exit-code`
(docs/PLAN.md §1.2 „Kontrakty”, §4.4).

| Plik | Źródło | Wersja |
|---|---|---|
| `event.v1.json` | `core-bus-contract::event_schema()` (`EVENT_SCHEMA_VERSION`) | 1 |
| `module-manifest.v1.json` | `core-registry-contract::manifest_schema()` (`MANIFEST_SCHEMA_VERSION`) | 1 |
| `ipc.v1.json` | `app-api::schema::ipc_schema()` (`IPC_SCHEMA_VERSION`) — komendy, ramka zdarzeń, błąd i wszystkie DTO łącza | 1 |

Aktualizacja po zmianie typów:
```bash
UPDATE_SCHEMAS=1 cargo test -p core-bus-contract -p core-registry-contract
UPDATE_SCHEMAS=1 cargo test -p app-api --test ipc_schema   # schemat łącza (ipc.v1.json)
node scripts/gen-ipc-types.mjs                             # typy TS z ipc.v1.json
git diff --exit-code packages/schemas   # w CI: musi być czysto
```
Zmiana niezgodna wstecz = nowy plik `*.v2.json` + upcaster + test migracji (PLAN §13); stare pliki zostają.
Typy TS dla UI powstają z `ipc.v1.json`: `node scripts/gen-ipc-types.mjs` zapisuje
`apps/desktop/ui/src/lib/api/generated/ipc.ts` (typy, mapa komend, ramka kanału terminala); `types*.ts` tylko
je przekazują, a ręczna kopia typu łącza zatrzymuje generator (B.1 etap 2). Pozostałe schematy
służą walidacji zdarzeń w testach, logom NDJSON i wtyczkom Wasm.
