# scripts/

| Skrypt              | Co robi                                                                                                                 | Gdzie używany        |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------- | -------------------- |
| `check-deps.sh`     | sprawdza przez `cargo metadata` + `jq`, że żaden crate `*-impl`/`*-fake` nie zależy od cudzego `*-impl` (tylko od `*-contract`; cudzy `*-fake` tylko w `dev-dependencies`); `--self-test` uruchamia wbudowany test reguł | CI (Linux), lokalnie |
| `pre-commit.sh`     | prettier, eslint, svelte-check, reguły Svelte 5, zakaz web fontów/backdrop-filter, aktualność tokenów                   | hook lokalny         |
| `check-svelte5.mjs` | wykrywa składnię Svelte 4 (`export let`, `$:`, `on:`)                                                                   | `pnpm lint`          |
| `check-css.mjs`     | wykrywa `@import url(` i `backdrop-filter` poza paletą poleceń                                                          | `pnpm lint`          |
| `setup-dev.ps1`     | Windows: sprawdza narzędzia do budowy Alfy; `-Install` (każdy krok za zgodą), `-Build`, `-Test` (także testy na żywym systemie, każdy za zgodą), `-Run`, `-Installer` — opis w `docs/user-guide/11-pierwszy-test-na-pc.md` | PC właściciela       |
| `ci/hf-gguf-check.py` | modele GGUF z `providers-local-impl/models.toml` w API Hugging Face (bez pobierania): plik istnieje, rozmiar = `size_mb` (±1 MiB), przypięty `sha256` = hash LFS; tabela SHA-256 do przypięcia przez człowieka; `HF_ENDPOINT` — atrapa API w testach | CI (`rehearsal.yml`, job „Hugging Face”) |

Skrypty są częścią bramek jakości z §4.4 planu (`docs/PLAN.md`).
