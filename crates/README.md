# crates/

Workspace Rust jądra i modułów Alfy (docs/PLAN.md §3.2, §3.6; ADR 0002).

## Zasada trójki crate'ów
Każdy moduł `<m>` to trzy crate'y:

| Crate | Zawartość | Kto może od niego zależeć |
|---|---|---|
| `<m>-contract` | traity, typy, nazwy zdarzeń, JSON Schema, współdzielone testy kontraktowe (feature `contract-tests`) | wszyscy |
| `<m>-impl` | implementacja produkcyjna + `module.toml` | tylko rejestr modułów / kompozycja builda |
| `<m>-fake` | deterministyczna atrapa (wirtualny zegar, record/replay) | **tylko `dev-dependencies`** testów innych modułów |

Reguła twarda: **moduł zależy od innego modułu wyłącznie przez `-contract`.** Zależność `-impl`/`-fake`
od cudzego `-impl` (w dowolnym rodzaju zależności) albo od cudzego `-fake` w zależnościach produkcyjnych
jest błędem CI — sprawdza to `scripts/check-deps.sh` (przez `cargo metadata` + `jq`).

## Crate'y w F0 (pkt 2 §4.5a)
| Moduł | Crate'y | Uwagi |
|---|---|---|
| `core-bus` | `core-bus-contract`, `core-bus-impl`, `core-bus-fake` | zdarzenia §13, schemat `event.v1.json` |
| `core-registry` | `core-registry-contract` | manifest `module.toml`, trait `Module`, schemat manifestu |
| `core-config` | `core-config-contract` | warstwy wspólna/maszyna, klucze-ścieżki, `ConfigStore` |
| `core-log` | `core-log-contract` | `LogSink` append-only, `AuditWriter` (Broker), `Redactor` |
| `platform-windows` | `platform-contract`, `platform-fake` | `SystemPort` neutralny; `-impl` (windows-rs) w F1 |
| `example-module` | `example-module-contract`, `-impl`, `-fake` | wzorzec dla wszystkich kolejnych modułów |

Brakujące `-impl`/`-fake` rejestru, konfiguracji i logów powstają w F0/F1 wg SPEC-ów w `docs/modules/`.

## Jak dodać moduł
1. `docs/modules/<m>/SPEC.md` (1 strona) → 2. `<m>-contract` (+ `contract_tests` pod feature) →
3. `<m>-fake` → 4. testy kontraktowe na fake → 5. `<m>-impl` z `module.toml` → 6. `scripts/check-deps.sh`,
`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
Wzór: skopiuj `example-module-*`. Wszystkie wersje zależności bierz z `[workspace.dependencies]`.

## Linty
`[workspace.lints]` w `Cargo.toml`: `unwrap_used`/`expect_used`/`todo`/`unimplemented` = deny
(w testach wyłączane przez `#![cfg_attr(test, allow(...))]` / `#![allow(...)]` na górze pliku testu),
`too_many_lines` = warn z progiem 300 (`clippy.toml`), `unsafe_code` = forbid (poza przyszłym `platform-windows-impl`
po ADR), `missing_docs` = warn. Plik ≤ 400 linii, crate ≤ 8 000 linii (AGENTS.md).
