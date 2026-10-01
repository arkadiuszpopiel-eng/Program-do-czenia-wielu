# ui-shell — SPEC (szkic v0)

## Cel
Powłoka UI (Tauri 2 + WebView2 + Svelte 5): okno główne z trzema warstwami (powierzchnia rozmowy, panele chowane, Ustawienia), pasek tytułu z obsadą i stanem, composer, kapsuła aktywności, karty/widok dzielony/odłączane okna, paleta `Ctrl+K`, skróty, tryb skupienia, responsywność, cykl życia okna vs zasobnik, IPC rdzeń → UI grupowane co klatkę (PLAN §14). Panele są osobnymi modułami UI ładowanymi leniwie.

## Fala i priorytet
F1 (okno główne, rozmowa, composer, panel Sesje, Ustawienia z manifestów, Oś czasu v0, paleta). Kolejne panele w falach ich modułów. P0.

## Kontrakt (szkic Rust)
```rust
// ui-shell-contract — SZKIC (komendy Tauri typowane, typy TS generowane tauri-specta/ts-rs)
pub struct WindowSpec { pub kind: WindowKind /* Main | DetachedPanel(PanelId) | QuickAsk | VoicePill | Settings */, pub capabilities: Vec<TauriCap> }
pub trait UiShell: Send + Sync {
    fn open(&self, spec: WindowSpec) -> Result<WindowId>;
    fn hide_main(&self) -> Result<()>;                       // do zasobnika; WebView niszczony po N min
    fn push_batch(&self, w: WindowId, events: Vec<UiEvent>) -> Result<()>;   // raz na klatkę
    fn command(&self, w: WindowId, cmd: UiCommand) -> Result<UiReply>;      // typowane komendy z UI do rdzenia
    fn render_markdown(&self, md: &str) -> SanitizedHtml;    // pulldown-cmark + ammonia, w Rust
}
pub enum UiEvent { TurnDelta { .. }, ToolStep { .. }, ApprovalPending { .. }, CostUpdate { .. }, StateChip { .. }, Toast { .. } }
pub struct PanelContribution { pub id: PanelId, pub title: I18nKey, pub icon: IconId, pub hotkey: Option<Hotkey>, pub lazy_chunk: String }
```
Zdarzenia: `ui.window.opened/hidden/destroyed`, `ui.panel.toggled`, `ui.focus_mode`, `ui.command` (paleta/skrót → komenda), `ui.perf.sample` (budżety §14.7 w buildzie testowym).

## Zależności
`core-bus/config/log-contract`, `sessions-contract`, `ui-kit` (pakiet TS + tokeny), `platform-windows-contract` (okna, Mica, Snap), `personas-contract` (F2), `safety-broker-contract` (F3: karta „czeka na zatwierdzenie" — bez samego zatwierdzania). Zewnętrzne: Tauri 2.x, Svelte 5 (runes), Bits UI, Vite (`docs/vendor/`).

## Niezmienniki
- Minimalne capabilities Tauri per okno; ścisłe CSP bez inline, Trusted Types; brak `eval`; port CDP tylko w buildzie testowym (test CI, PLAN §8.2).
- Markdown z LLM renderowany w Rust do sanitizowanego HTML; artefakty HTML/SVG tylko w sandboxowanym iframe bez IPC.
- Zatwierdzenia nigdy w tym oknie — tylko przekierowanie do Broker-UI.
- Aktualizacja DOM najwyżej raz na klatkę; lista wiadomości wirtualizowana; podświetlanie składni w Web Workerze po zamknięciu bloku; duże dane przez ścieżki/protokół zasobów, nie IPC.
- Składnia Svelte 5: zakaz `export let`, `$:`, `on:` (lint w CI); brak SSR; brak fontów webowych; `backdrop-filter` tylko na palecie.
- Reguła AltGr dla skrótów; `F5`/`Ctrl+R` wyłączone; wszystko osiągalne z klawiatury (WCAG 2.2 AA, axe w CI).
- Zamknięcie okna = ukrycie (WebView żyje N min), potem zniszczenie; ponowne otwarcie ciepłe ≤ 150 ms, zimne ≤ 1 s.

## Zdolności / uprawnienia
Brak tokenów Brokera; akcje użytkownika z UI (Otwórz, Zapisz jako) wykonywane jako Ty przez `SystemPort`.

## Izolacja
`inproc` (Rust) + proces WebView2 (wspólne środowisko dla wszystkich okien — jeden proces przeglądarki).

## Budżet zasobów
§14.7: composer ≤ 16 ms p95; strumień 100 tok/s płynne 60 kl./s; panel ≤ 100 ms; przełączenie sesji 1000 wiadomości ≤ 150 ms; paleta ≤ 50 ms; JS startowy ≤ 150 KB gzip, CSS ≤ 30 KB; idle ~0% CPU. RAM drzewa procesów — cel z F0(f).

## Konfiguracja (klucze TOML)
`[ui] theme = "auto"`, `density = "comfortable"`, `zoom = 100`, `column = "narrow" | "wide"`, `enter_sends = true`, `focus_mode_hotkey = "F11"`, `hide_to_tray = true`, `destroy_webview_after = "10m"`, `max_detached_windows = 4`; per maszyna: szerokości paneli, pozycja okna, monitor; per sesja: otwarte panele.

## Wkład do UI
To jest UI: makiety 1, 2, 7, 11, 18 (§14.10); rejestracja paneli z `PanelContribution`; strony ustawień generowane z manifestów modułów.

## Testy akceptacyjne
- `ACC-F1-ui-shell-01`: budżety §14.7 zmierzone śladem CDP/Playwright pod limitami baseline (desktop z emulacją) — wszystkie w progu.
- `ACC-F1-ui-shell-02`: axe: 0 naruszeń critical/serious; nawigacja klawiaturą E2E na makietach 1–2.
- `ACC-F1-ui-shell-03`: przyrost Private WS ≤ 5% po 1 h / 500 wiadomościach.
- `ACC-F1-ui-shell-04`: test CI — port CDP zamknięty w buildzie produkcyjnym; CSP bez inline.

## Fake
Frontend testowany z atrapą `FakeAlfaClient` (`apps/desktop/ui/src/lib/api/fake/`, Storybook + Playwright): skryptowane strumienie (~100 tok/s, wirtualny zegar w testach), kroki narzędzi, karty zatwierdzeń, warianty/gałęzie, scenariusze `offline`, `rate-limited` (429 z czasem odnowienia), `no-keys`, `first-run`, `no-mic`, `disk-low`, `empty`. Docelowo ten sam kontrakt zasila most do `core-bus-fake`.

## Warstwa danych UI (F1)
- Interfejs `AlfaClient` (`src/lib/api/client.ts`) z dwiema implementacjami: `TauriAlfaClient` (cienki adapter `invoke`/`listen`) i `FakeAlfaClient`; wybór: `window.__TAURI_INTERNALS__` ? Tauri : atrapa (atrapa ładowana leniwie, poza paczką startową).
- Kontrakt IPC (komendy `snake_case`, jeden kanał zdarzeń `alfa://events` z paczkami `AlfaEvent[]`, nazwy strumienia `TurnAppended/TextDelta/ThinkingDelta/ToolCall/ApprovalPending/Usage/Stop/Error`): `apps/desktop/ui/src/lib/api/COMMANDS.md` — do zaimplementowania w `src-tauri`. Typy DTO w `types*.ts` (pola `snake_case`) zostaną zastąpione generowanymi (ADR 0013).
- Strumień: `TextDelta.blocks[].html_sanitized` z Rust; UI wstawia HTML wyłącznie w `SanitizedHtml.svelte`. Zdarzenia buforowane w `RafBatcher` — stan i DOM aktualizowane najwyżej raz na klatkę; `aria-live` ogłasza pełne zdania z throttlingiem (`SentenceAnnouncer`).
- Lista wiadomości wirtualizowana (`VirtualList`, pomiar per klucz, ± 1 ekran), rola `feed`; podświetlanie składni w Web Workerze (tokenizer bez zależności) po zamknięciu bloku.
- Leniwie (`import()`): panele prawe, Ustawienia, wprowadzenie, paleta (wstępnie pobierana w bezczynności; otwarcie = `dialog.show()`), ściągawka, słownik EN, atrapa.
- i18n PL/EN od dnia 0: `src/lib/i18n` (słowniki `pl*.ts`/`en*.ts`, liczby mnogie `Intl.PluralRules`, formaty pl-PL przez `Intl`); kompletność kluczy sprawdza TypeScript i test.
- Skróty: rejestr z §14.8 (`logic/shortcut-registry.ts`), wykrywanie konfliktów (duplikat, AltGr, zarezerwowane), nadpisania użytkownika; „karty" w F1 = ostatnio używane sesje (Ctrl+Tab, Ctrl+1…9, Ctrl+W, Ctrl+Shift+T). Zoom przez CSS `zoom` (80–200 %), responsywność z szerokości efektywnej.

## Weryfikacja (F1, UI)
`pnpm --filter @alfa/desktop-ui test` (vitest: logika, store'y z runami, i18n, atrapa), `test:e2e` (Playwright + axe na zbudowanym UI: 0 naruszeń critical/serious na ekranach w motywie jasnym i ciemnym, klawiatura, paleta ≤ 50 ms, DOM raz na klatkę przy 100 tok/s), `bundle-size` (statyczny graf importów z manifestu Vite: start JS ≤ 150 KB, CSS ≤ 30 KB gzip).

## Implementacja F1 — rdzeń i powłoka (stan)
- **Rdzeń komend:** `crates/app-core` (korzeń kompozycji `app-*`): każda komenda z COMMANDS.md jako
  `AppCore::<przestrzeń>_<nazwa>`; DTO serde 1:1 z `types*.ts` (test round-trip na ładunkach atrapy UI);
  lista komend jednym źródłem — makro `app_core::with_commands!` (handlery Tauri + test sygnatur `Send`).
- **Zdarzenia:** `EventHub` grupuje zdarzenia w paczki raz na klatkę (16 ms, `AppOptions::frame`),
  scala sąsiednie `TextDelta` tej samej tury i `MicLevel`; brak zdarzeń = brak wybudzeń. Powłoka emituje
  paczkę na `alfa://events`.
- **Strumień:** delty dostawcy → `lib-markdown::IncrementalRenderer` → `TextDelta.blocks[]` (zamknięte +
  otwarty, `kind: code` dla bloku kodu najwyższego poziomu z `lang`); anulowanie (`turns_stop`, kill-switch)
  przez `CancellationToken` z `select!` (≤ 100 ms niezależnie od dostawcy).
- **Okna (Tauri):** tworzone w `setup` na wspólnym, stałym folderze danych WebView2
  (`%LOCALAPPDATA%\Alfa\webview-data`): `main` (natywne dekoracje do czasu spike'u j — Snap Layouts),
  `quick` (640 px, bez dekoracji, przezroczyste, ukryte, chowane przy utracie fokusu), `pill` (220×48, zawsze
  na wierzchu, ukryte). Zamknięcie `main` = ukrycie; po `general.destroy_webview_after` min WebView jest
  niszczony i odtwarzany przy pokazaniu (`general.close_to_tray = false` → wyjście).
- **Bezpieczeństwo:** capabilities per okno (`capabilities/{main,quick,pill}.json`, bez `core:default`,
  uprawnienia `allow-<komenda>` z manifestu aplikacji w `build.rs`); CSP mapą dyrektyw bez `unsafe-inline`
  dla skryptów; DevTools tylko w debug; port CDP wyłącznie z cechą `e2e`. Trusted Types — jeszcze nie
  wymuszane w CSP (wymaga polityki dla `SanitizedHtml.svelte`).
- **Luka kontraktu:** brak zdarzenia „przejdź do sesji" — „Nowa rozmowa" z zasobnika/protokołu ustawia
  aktywną sesję (widoczną po starcie UI) i wysyła `SessionUpdated`, ale działające UI nie przełącza widoku.

## Otwarte pytania
- Snap Layouts i Mica z własnym paskiem tytułu w Tauri (spike j): UI ma region `data-tauri-drag-region` i rezerwuje miejsce na natywne przyciski (`--alfa-titlebar-controls`); Playwright przez CDP vs `tauri-driver` — do ustalenia po F0.
- Pisownia PL w WebView2 (spike j) — composer ma `spellcheck` i `lang` z bieżącego języka.
- Odłączanie paneli do osobnych okien i widok dzielony — po F1 (panele są już osobnymi modułami ładowanymi leniwie).
