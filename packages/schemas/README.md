# packages/schemas/

JSON Schema **generowane z Rust** (schemars) — nie edytuj ręcznie. Źródło prawdy to typy w crate'ach
`*-contract`; plik w tym katalogu jest snapshotem, który CI porównuje przez `git diff --exit-code`
(docs/PLAN.md §1.2 „Kontrakty”, §4.4).

| Plik | Źródło | Wersja |
|---|---|---|
| `event.v1.json` | `core-bus-contract::event_schema()` (`EVENT_SCHEMA_VERSION`) | 1 |
| `module-manifest.v1.json` | `core-registry-contract::manifest_schema()` (`MANIFEST_SCHEMA_VERSION`) | 1 |

Aktualizacja po zmianie typów:
```bash
UPDATE_SCHEMAS=1 cargo test -p core-bus-contract -p core-registry-contract
git diff --exit-code packages/schemas   # w CI: musi być czysto
```
Zmiana niezgodna wstecz = nowy plik `*.v2.json` + upcaster + test migracji (PLAN §13); stare pliki zostają.
Typy TS dla UI generuje osobno `tauri-specta`/`ts-rs` (apps/desktop); schematy tutaj służą walidacji
zdarzeń w testach, logom NDJSON i wtyczkom Wasm.
