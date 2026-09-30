# ADR 0001 — Stos: Rust + Tauri 2 + Svelte 5

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.2 (Platforma, Rdzeń, Powłoka UI, Frontend), §3.4, §14.7, §17 |

## Kontekst

Alfa ma być lekkim, bardzo modułowym programem dla Windows 11 x64 z audio w czasie rzeczywistym (WASAPI, AEC, barge-in), bogatym UI z chowanymi panelami i wieloma oknami, a cały kod piszą modele AI (Opus 5.5 / GPT-6 Sol). Potrzebny jest stos, który:

- daje bezpieczeństwo pamięci i deterministyczne wątki RT dla audio,
- dostarcza kompilator jako pętlę zwrotną dla kodu generowanego przez AI,
- pozwala na nowoczesny, dostępny UI bez ciężkiego runtime,
- działa na baseline (Ryzen 5 5600, 16 GB RAM, RX 7600).

## Decyzja

| Warstwa | Wybór | Uwagi |
|---|---|---|
| Platforma | Windows 11 x64; Win10 best effort; ARM64 poza v1 | rzeczy OS-specyficzne za traitem `SystemPort` w crate `platform-windows` |
| Rdzeń | Rust, jeden workspace | LTO, strip; opt-level pod RT tylko tam, gdzie trzeba |
| Powłoka UI | Tauri 2.x + WebView2 | zostajemy na 2.x — Tauri 3 jest w alfie |
| Frontend | Svelte 5 (runes) + Bits UI, czysty Vite, bez SSR | w CI `svelte-check` + `eslint-plugin-svelte`; w AGENTS.md zakaz `export let`, `$:`, `on:`; oficjalny Svelte MCP jako kontekst dla modeli |
| Język UI | polski + angielski, i18n od dnia 0 | |

Uczciwe zastrzeżenie: RAM to głównie WebView2 (Chromium). **Nie obiecujemy „5× mniej niż Electron"**. F0 (spike f) mierzy sumę Private Working Set całego drzewa procesów przy 1 i 3 oknach; zamknięte okno jest ukrywane przez N minut (domyślnie 10), potem WebView jest niszczony.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Electron | cięższy runtime i osobny Node; brak korzyści wobec WebView2 obecnego w Windows 11 |
| Tauri 3 | alpha; wiedza modeli AI kończy się przed jego stabilizacją |
| React 19 + React Aria | **plan B**, gdyby modele nie utrzymały poprawnej składni Svelte 5 mimo lintów; cięższy bundle |
| Solid | odradzany — 2.0 w RC |
| Natywne UI (WinUI 3 / egui) | WinUI wymaga C#/C++ lub niedojrzałych bindingów; egui nie daje dostępności i typografii WCAG 2.2 AA |
| C++/C# jako rdzeń | brak gwarancji pamięci; słabsza pętla zwrotna dla AI (Rust: typy + borrow checker filtrują halucynacje) |

## Konsekwencje

- Kontrakty Rust → typy TS generowane (ADR 13); JSON Schema zdarzeń; brak dryfu między warstwami.
- Budżety UI (§14.7 planu) mierzone w CI przez ślad CDP/Playwright pod limitami baseline.
- Ryzyko: modele mieszają składnię Svelte 4/5 → egzekwowane lintami i hookami, nie prozą.
- Ryzyko: WebView2 dominuje w RAM → budżet „idle z oknem" ustalony dopiero po pomiarze w F0.
- windows-rs w jednej wersji, w jednym crate.

## Jak cofnąć

- Frontend: przejście na React 19 + React Aria jest przewidziane jako plan B; kontrakty IPC (typy generowane) pozostają, wymiana dotyczy tylko `apps/desktop/ui/` i `packages/ui-kit`.
- Powłoka: `SystemPort` i moduły w Rust nie zależą od Tauri; wymiana powłoki dotyczy `apps/desktop/` i mostka IPC.
- Decyzja odwracalna do końca F0 (jak każda z §1.2 planu).
