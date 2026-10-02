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

## Wtyczki Tauri i ich rola
`tauri-plugin-single-instance` (pierwsza: druga instancja przekazuje URI `alfa://`), `tauri-plugin-global-shortcut`
(`Ctrl+Alt+Space`, kill-switch `Ctrl+Shift+F12` → `system_kill_all(KillOrigin::Hotkey)` → Broker),
`tauri-plugin-notification` (toasty Windows z `app_core::notify`), `tauri-plugin-dialog` (=2.8.1: natywne
dialogi „Zapisz jako", wyboru paczki `.alfa` i katalogu roboczego agentek dla portu `ShellPort`; wołane
z `spawn_blocking`). „Uruchom w terminalu" (`ShellPort::open_terminal`): nowe okno konsoli (Windows 11 —
w domyślnym terminalu) z `pwsh`/`powershell`/`cmd` w katalogu kroku; katalog jest katalogiem bieżącym
procesu (nie argumentem), polecenie nie jest przekazywane ani wykonywane — właściciel wkleja je sam.
Każda nowa wtyczka/zależność powłoki trafia do `src-tauri/Cargo.lock` (osobny lockfile — root `cargo deny`
go nie obejmuje). Licencje i źródła: `cargo deny --manifest-path apps/desktop/src-tauri/Cargo.toml check
licenses sources` (zielone); `bans` pada na samym Tauri (wry/tao/webview2-com ciągną `windows`, który
`deny.toml` dopuszcza tylko w `platform-windows-impl`) — znany wyjątek powłoki, nie ruszać bez ADR.

## Terminal i ochrona okien (F8)
Wbudowany terminal: `terminal_open` ma handler ręczny (`app_core::CHANNEL_COMMANDS`) z argumentem
`tauri::ipc::Channel<TerminalFrame>` — ramki VT (base64) idą tylko tym kanałem do okna, które otworzyło
terminal (nigdy przez `alfa://events`, magistralę ani logi); `terminal_input`/`resize`/`close`/`list`
— zwykłe komendy z `with_commands!`, tylko w `capabilities/main.json`. Emulator w UI: `@xterm/xterm`
6.0.0 + `@xterm/addon-fit` 0.11.0 (MIT, bez zależności), ładowany leniwie z dialogiem terminala.
Okna tworzone z `content_protected(true)` (`WDA_EXCLUDEFROMCAPTURE`) — niewidoczne na zrzutach ekranu,
także dla narzędzi computer use agentek.

## Weryfikacja powłoki poza Windows
Job CI „Powłoka Tauri (Windows)" (`.github/workflows/ci.yml`) robi `cargo fmt --check` i `cargo clippy
--all-targets [--features e2e] -- -D warnings` z prawdziwym `app-core`. Na Linuksie `cargo fmt --check`
działa wprost; `cargo clippy --target x86_64-pc-windows-msvc` wymaga kopii powłoki z **zaślepką
`app-core`** (te same typy z `app-api` i sygnatury z `with_commands!`, bez modułów z zależnościami C —
SQLCipher/OpenSSL, whisper.cpp — których nie da się skompilować krzyżowo bez MSVC). Zaślepka sprawdza
tylko kod powłoki; zgodność z prawdziwym rdzeniem potwierdza job Windows.

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
    shortcuts.rs, pump.rs, shell.rs — ShellPort: okna, dialogi, terminal, OpenSession), build.rs,
    icons/ (placeholdery); logika w crates/app-core (+ app-api: DTO/porty, app-modules: adaptery modułów,
    app-agents: agentki z narzędziami, app-voice: tryb głosowy)
```
