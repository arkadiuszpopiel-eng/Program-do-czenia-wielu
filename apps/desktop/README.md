# apps/desktop — powłoka Alfy (Tauri 2 + Svelte 5)

Okno główne Alfy: `src-tauri/` (Rust, Tauri 2.x, WebView2) + `ui/` (Svelte 5 z runes, czysty Vite, bez SSR).
UI używa pakietu `@alfa/ui-kit` (`packages/ui-kit`) — tokeny designu i komponenty; Storybook z makietami tam.

## Wymagania (Windows 11 x64 — cel produktu)
- Rust 1.94 (toolchain **MSVC**): `rustup default stable-x86_64-pc-windows-msvc`
- Visual Studio Build Tools 2022 z „Desktop development with C++" (MSVC + Windows 11 SDK)
- WebView2 Runtime (wbudowany w Windows 11; instalator NSIS dociąga bootstrapper)
- Node 22 + pnpm 10 (`corepack enable` albo `npm i -g pnpm@10`)
- (opcjonalnie) NSIS do paczki instalacyjnej — `tauri build` pobierze go sam

Na Linuksie/macOS frontend buduje się i przechodzi wszystkie bramki bez Tauri; sama powłoka wymaga
WebKitGTK (Linux) / WKWebView (macOS) i **nie jest tu celem** — CI kompiluje Rust na `windows-latest`.

## Uruchomienie
```powershell
pnpm install                       # w katalogu głównym repo
pnpm --filter @alfa/desktop-ui dev # sam frontend: http://localhost:1420
cd apps/desktop/src-tauri
cargo tauri dev                    # okno Tauri (wymaga: cargo install tauri-cli --version "^2")
# albo bez globalnego CLI:
pnpm --filter @alfa/desktop-ui exec tauri dev
```

## Build produkcyjny
```powershell
pnpm --filter @alfa/desktop-ui build        # ui/dist
node apps/desktop/ui/scripts/bundle-size.mjs # budżet: JS ≤ 150 KB gzip, CSS ≤ 30 KB gzip (exit 1 przy przekroczeniu)
cd apps/desktop/src-tauri
cargo tauri build                            # target/release/bundle/nsis/Alfa_*.exe
```

## Workspace Cargo
`src-tauri/Cargo.toml` ma **pustą sekcję `[workspace]`**, więc jest samodzielnym pakietem.
Workspace Rust w katalogu głównym repo (crates `core-*`, moduły) ma go wykluczyć wpisem:
```toml
[workspace]
exclude = ["apps/desktop/src-tauri"]
```
Powód: Tauri ciągnie duże drzewo zależności (wry/tao/webview2-com) i osobne cele budowania;
nie chcemy, by `cargo clippy --workspace` na Linuksie próbował je kompilować. Ikony w `src-tauri/icons/`
są placeholderami — docelowe wygeneruj: `cargo tauri icon sciezka/do/logo.png`.

## Okno i pasek tytułu
`tauri.conf.json`: okno 1200×800, min. 400×500, **natywne dekoracje** (`decorations: true`).
TODO F0(j): własny pasek tytułu (`decorations: false` + `data-tauri-drag-region`) z natywnymi przyciskami
i obsługą **Snap Layouts** Windows 11 — do sprawdzenia w Tauri 2 (PLAN.md §14.2). Tło Mica: również F0(j).
Uprawnienia są osobne dla każdego okna (`capabilities/main.json`, `quick.json`, `pill.json`; PLAN §8.2):
każde okno dostaje tylko komendy, których używa (lista generowana w `build.rs` z `ui/src/lib/api/COMMANDS.md`);
każde nowe uprawnienie to osobna decyzja (AGENTS.md → Broker).

## Bramki jakości UI (lokalnie, przed commitem)
```bash
bash scripts/pre-commit.sh
```
Uruchamia po kolei: `prettier --check`, `eslint` (reguły Svelte 5: zakaz `export let`, `$:`, `on:`,
`createEventDispatcher`), `svelte-check` (`pnpm check`), `scripts/check-svelte5.mjs`, `scripts/check-css.mjs`
(zakaz `@import url(`, `@font-face`, web fontów; `backdrop-filter` tylko w `CommandPalette.svelte`),
`build:tokens` + `git diff --exit-code` na wygenerowanych `packages/ui-kit/src/tokens.{css,ts}`, `check:contrast` (WCAG).
Instalacja jako hook git (bez `.claude/settings.json`):
```bash
cp scripts/pre-commit.sh .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
# Windows (Git for Windows uruchomi skrypt basha):
copy scripts\pre-commit.sh .git\hooks\pre-commit
```
Te same kroki wykonuje job `ui` w `.github/workflows/ci.yml`.

## Skrypty w katalogu głównym
| Skrypt | Co robi |
|---|---|
| `pnpm check` | `svelte-check --fail-on-warnings` we wszystkich pakietach |
| `pnpm lint` | eslint + check-svelte5 + check-css |
| `pnpm build` | `pnpm -r build` (tokeny ui-kit + Vite build UI) |
| `pnpm storybook` / `pnpm build-storybook` | Storybook `@alfa/ui-kit` (dev na :6006 / `packages/ui-kit/storybook-static`) |
| `pnpm --filter @alfa/ui-kit build:tokens` | regeneruje `tokens.css`/`tokens.ts` z `src/tokens/tokens.json` |
| `pnpm --filter @alfa/ui-kit check:contrast` | tabela kontrastu WCAG dla wszystkich kolorów, exit 1 przy naruszeniu |

## Struktura
```
apps/desktop/
  README.md
  ui/                 Vite + Svelte 5 (index.html, src/main.ts, src/App.svelte — makieta Rozmowy z atrapami)
    scripts/bundle-size.mjs
  src-tauri/          Cargo.toml (tauri = 2.12.0, samodzielny [workspace]), tauri.conf.json,
    capabilities/{main,quick,pill}.json, src/ (lib.rs, commands.rs, windows.rs, tray.rs,
    shortcuts.rs, pump.rs, shell.rs), build.rs, icons/ (placeholdery); logika w crates/app-core
```
