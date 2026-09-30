# scripts/

| Skrypt              | Co robi                                                                                                                 | Gdzie używany        |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------- | -------------------- |
| `check-deps.sh`     | sprawdza przez `cargo metadata` + `jq`, że żaden crate `*-impl`/`*-fake` nie zależy od cudzego `*-impl` (tylko od `*-contract`; cudzy `*-fake` tylko w `dev-dependencies`); `--self-test` uruchamia wbudowany test reguł | CI (Linux), lokalnie |
| `pre-commit.sh`     | prettier, eslint, svelte-check, reguły Svelte 5, zakaz web fontów/backdrop-filter, aktualność tokenów                   | hook lokalny         |
| `check-svelte5.mjs` | wykrywa składnię Svelte 4 (`export let`, `$:`, `on:`)                                                                   | `pnpm lint`          |
| `check-css.mjs`     | wykrywa `@import url(` i `backdrop-filter` poza paletą poleceń                                                          | `pnpm lint`          |

Skrypty są częścią bramek jakości z §4.4 planu (`docs/PLAN.md`).
