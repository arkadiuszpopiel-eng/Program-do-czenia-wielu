# AGENTS.md — reguły dla agentów AI budujących Alfę

Ten plik zawiera tylko to, czego nie da się wyprowadzić z kodu. Źródło prawdy: `docs/PLAN.md`.
Egzekucja reguł: hooki i CI (nie proza). Zmiany tego pliku zatwierdza człowiek.

## Stos (wersje przypięte w `Cargo.toml` / `package.json`; nie podnosić bez ADR)
- Rust (workspace, edycja 2024, toolchain MSVC na Windows). Cel: Windows 11 x64; kod OS-specyficzny tylko w `platform-windows` za traitem `SystemPort`.
- Tauri **2.x** (nie 3.x alpha) + WebView2. Frontend: **Svelte 5 z runes** + Bits UI, czysty Vite, bez SSR.
- Zakaz składni Svelte 4: `export let`, `$:`, `on:click`. Używaj `$props()`, `$state()`, `$derived()`, `$effect()`, `onclick`.
- Audio: crate `wasapi` (nie `cpal`). ML bez Pythona: whisper.cpp (Vulkan/CUDA/CPU), ONNX Runtime CPU, sherpa-onnx, llama.cpp.
- windows-rs: jeden crate, jedna wersja. Wiedza modeli o WASI 0.3, wasmtime 46, tauri-specta RC jest nieświeża → czytaj `docs/vendor/<crate>.md`.

## Format modułu
- Moduł = trójka crate'ów: `<m>-contract` (trait + typy + zdarzenia + JSON Schema), `<m>-impl`, `<m>-fake`.
- Inne moduły zależą **wyłącznie od `<m>-contract`**. Zależność od `-impl` innego modułu = błąd CI.
- Każdy moduł ma `module.toml` (id, version, kind, kontrakty, zdolności, budżet RAM/CPU, cykl życia, izolacja, health-check) i `docs/modules/<m>/SPEC.md` (1 strona).
- Kontrakty Rust → typy TS generowane (tauri-specta / ts-rs). Plików generowanych nie edytuj ręcznie; CI sprawdza `git diff --exit-code`.

## Reguły twarde (test w CI)
- Żadnych fontów webowych; fonty systemowe (Segoe UI Variable, Cascadia Mono, `system-ui`).
- CSS `backdrop-filter` zabroniony poza jedną nakładką (paleta `Ctrl+K`). Tła okna przez natywną Mica.
- Żaden skrót globalny `Ctrl+Alt(+Shift)` + litera a, c, e, l, n, o, s, x, z (polski AltGr). Kill-switch: `Ctrl+Shift+F12`.
- Markdown z LLM renderowany do HTML **w Rust** (pulldown-cmark + ammonia), bez surowego HTML/skryptów; UI tylko wstawia wynik. Iframe bez IPC tylko dla artefaktów HTML/SVG.
- Historia rozmowy jest **append-only**; „edytuj" i „ponów" tworzą nowe gałęzie, nigdy nie modyfikują wcześniejszych tur.
- Broker (`safety-broker`, `broker-ui`), watchdog, audyt, kill-switch, deny-listy, tagi prywatności są **poza zasięgiem agentek** i poza zasięgiem Ulepszacza. Zakaz `gui.control` wobec procesów Alfy/Brokera.
- Kod Alfy nigdy nie czyta ani nie przechowuje tokenów CLI (`~/.claude`, `~/.codex`) ani ciasteczek przeglądarek.
- Zero `TODO`, `FIXME`, `unimplemented!()`, `unwrap()`/`expect()` w ścieżkach produkcyjnych; zero ostrzeżeń.
- Sekrety tylko w Windows Credential Manager; nigdy w plikach konfiguracji, logach ani eksporcie `.alfa`.

## Kolejność pracy nad modułem
1. `SPEC.md` (pisze autor, akceptuje model-recenzent; Jądro/Broker — człowiek).
2. `-contract` (trait, typy, zdarzenia, schema).
3. `-fake` (deterministyczna atrapa; fake audio z wirtualnym zegarem, fake LLM record/replay, fake `SystemPort`).
4. Testy najpierw: kontraktowe (uruchamiane na `-fake` i `-impl`), jednostkowe, property-based.
5. Implementacja `-impl`.
6. Przegląd drugiego modelu w świeżym kontekście (ograniczony do poprawności, bezpieczeństwa, zgodności ze SPEC).
7. CI zielone → merge (automerge; Jądro/Broker/polityki bezpieczeństwa dodatkowo przez człowieka).

## Definition of Done (skrót; pełna w `docs/AI_WORKFLOW.md`)
SPEC aktualny · kontrakt + fake + testy kontraktowe zielone · pokrycie ≥ 85% linii / ≥ 70% gałęzi (nie-UI) · regresja dla każdego błędu ·
zero TODO/unwrap/ostrzeżeń · budżety RAM/CPU/opóźnień zmierzone na baseline emulowanym · stany brzegowe i błędy pokryte ·
przegląd drugiego modelu zamknięty · lista bezpieczeństwa + dokumentacja ustawień · merge tylko na zielono.

## Limity rozmiaru (egzekwowane lintem)
- Plik ≤ 400 linii, crate ≤ 8 000 linii, funkcja: clippy `too_many_lines`. Moduł ma mieścić się w małym oknie kontekstu.
- Jedna sesja AI = jeden moduł = jeden git worktree = jedna karta zadania (GitHub Issue). Nie dotykaj plików innych modułów.

## Komendy
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cd apps/desktop/ui && npm run check      # svelte-check + eslint
cd apps/desktop/ui && npm run test       # vitest / Playwright
```
Testy sprzętowe (audio, GPU, UIA) uruchamia self-hosted runner na maszynach użytkownika; lokalnie w chmurze używaj fake'ów.

## Gdzie szukać
| Pytanie | Plik |
|---|---|
| Co i dlaczego budujemy | `docs/PLAN.md` |
| Jak wygląda architektura, magistrala, moduły | `docs/ARCHITECTURE.md` |
| Jak pracujemy (sesje, przegląd, CI, DoD, szablony) | `docs/AI_WORKFLOW.md` |
| Progi akceptacyjne fali | `docs/ACCEPTANCE.md`, zamrożone zestawy w `evals/` |
| Model zagrożeń i twarde blokady | `docs/THREAT_MODEL.md` |
| Kontrakt i testy konkretnego modułu | `docs/modules/<m>/SPEC.md` |
| Jak używać zależności po cutoffie wiedzy | `docs/vendor/<crate>.md`, docs.rs |
| Decyzje architektoniczne | `docs/ADR/` |

## Język
- Komentarze, dokumentacja, komunikaty UI i nazwy w SPEC: **polski**. Identyfikatory w kodzie: angielski.
- Agentki mówią w rodzaju żeńskim („zrobiłam", „sprawdziłam"). i18n PL/EN od dnia 0.

## Czego nie wolno
- Commitów do `crates/core-*`, `safety-broker`, `broker-ui`, `watchdog`, `updater` i polityk bezpieczeństwa bez przeglądu człowieka.
- Zmiany progów w `evals/` (zestawy zamrożone hashem) i zmiany `AGENTS.md` bez zatwierdzenia człowieka.
- Dodawania crate'a bez wpisu w `Cargo.lock` sprawdzonego `cargo deny` (halucynowane crate'y).
- Uruchamiania mostów CLI (`claude`, `codex`) z harmonogramu; logowanie do nich wykonuje wyłącznie człowiek.
