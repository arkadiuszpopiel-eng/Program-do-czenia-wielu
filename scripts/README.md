# scripts/

| Skrypt              | Co robi                                                                                                                 | Gdzie używany        |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------- | -------------------- |
| `check-deps.sh`     | sprawdza przez `cargo metadata` + `jq`, że żaden crate `*-impl`/`*-fake` nie zależy od cudzego `*-impl` (tylko od `*-contract`; cudzy `*-fake` tylko w `dev-dependencies`); `--self-test` uruchamia wbudowany test reguł | CI (Linux), lokalnie |
| `pre-commit.sh`     | prettier, eslint, svelte-check, reguły Svelte 5, zakaz web fontów/backdrop-filter, aktualność tokenów                   | hook lokalny         |
| `check-svelte5.mjs` | wykrywa składnię Svelte 4 (`export let`, `$:`, `on:`)                                                                   | `pnpm lint`          |
| `check-css.mjs`     | wykrywa `@import url(` i `backdrop-filter` poza paletą poleceń                                                          | `pnpm lint`          |
| `setup-dev.ps1`     | Windows: sprawdza narzędzia do budowy Alfy; `-Install` (każdy krok za zgodą), `-Build`, `-Test` (także testy na żywym systemie, każdy za zgodą), `-Run`, `-Installer` — opis w `docs/user-guide/11-pierwszy-test-na-pc.md` | PC właściciela       |
| `install-on-drive.ps1` | Windows, świeży komputer: jedno polecenie instaluje wszystko na wskazanym dysku (`-Drive D`) — repozytorium `D:\alfa`, narzędzia `D:\alfa-narzedzia` (Rust, VS Build Tools, Git, Node.js, Perl, cache npm/pnpm; uprawnienia tylko właściciel/SYSTEM/Administratorzy), potem `setup-dev.ps1 -Build` i trzy skróty na Pulpicie (Alfa, aktualizacja kodu, praca nad kodem z Claude Code); `-Yes` bez pytań, `-Ci` tylko plan. Dane Alfy celowo zostają w `%LOCALAPPDATA%\Alfa` (ochrona Brokera po ścieżce) | PC właściciela, CI (`rehearsal.yml`, `-Ci`) |
| `update-dev.ps1`    | Windows: `git pull --ff-only` (bez nadpisywania lokalnych zmian) + `setup-dev.ps1 -Build`; skrót „Alfa — aktualizuj kod” | PC właściciela, CI (składnia) |
| `code-session.ps1`  | Windows: Claude Code (`claude`) w katalogu repozytorium — praca nad kodem przy działającej Alfie w trybie deweloperskim; skrót „Alfa — praca nad kodem (Claude Code)” | PC właściciela, CI (składnia) |
| `ci/hf-gguf-check.py` | modele GGUF z `providers-local-impl/models.toml` w API Hugging Face (bez pobierania): plik istnieje, rozmiar = `size_mb` (±1 MiB), przypięty `sha256` = hash LFS; tabela SHA-256 do przypięcia przez człowieka; `HF_ENDPOINT` — atrapa API w testach | CI (`rehearsal.yml`, job „Hugging Face”) |

Skrypty są częścią bramek jakości z §4.4 planu (`docs/PLAN.md`).
