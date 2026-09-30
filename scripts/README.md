# scripts/

| Skrypt | Co robi | Gdzie używany |
|---|---|---|
| `check-deps.sh` | sprawdza przez `cargo metadata`, że żaden crate `*-impl`/`*-fake` nie zależy od innego `*-impl` (tylko od `*-contract`) | CI (Linux), lokalnie |
| `pre-commit.sh` | prettier, eslint, svelte-check, reguły Svelte 5, zakaz web fontów/backdrop-filter, aktualność tokenów | hook lokalny |
| `check-svelte5.mjs` | wykrywa składnię Svelte 4 (`export let`, `$:`, `on:`) | `pnpm lint` |
| `check-css.mjs` | wykrywa `@import url(` i `backdrop-filter` poza paletą poleceń | `pnpm lint` |

Skrypty są częścią bramek jakości z §4.4 planu (`docs/PLAN.md`).
