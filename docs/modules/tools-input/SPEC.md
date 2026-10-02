# tools-input — SPEC (v1: narzędzia zaimplementowane, F5/F6)

## Cel
Wejście syntetyczne agentek (`SendInput`): tekst Unicode, skróty, kliknięcia (punkt w oknie albo środek elementu UIA), przewijanie — ostatnia trasa z PLAN §7.1. **Nigdy** do okien Alfy/Brokera/helpera; fizyczne wejście użytkownika ma pierwszeństwo (§7.4).

## Fala i priorytet
F5 (v1.5: tekst — dyktowanie), F6 (v2: skróty, mysz), P1.

## Kontrakt
```rust
input_type_text { window, text }                                   → InputOutput { window, app, batches, events, verified }
input_keys      { window, keys: ["Ctrl+S", …] (1–10) }               → InputOutput
input_click     { window, x?, y? | element?, button?, double? }      → InputOutput   // x,y względem okna
input_scroll    { window, notches (±1–20), horizontal?, x?, y? }      → InputOutput
pub struct InputTools; impl InputTools { pub fn new(InputToolsDeps { desktop, uia, input: InputPort, broker, config, bus }) }
```
Zdarzenia: `tool.input.sent` (narzędzie, okno, aplikacja, liczba paczek/zdarzeń — **nigdy tekst**), `tool.gui.verify`.

## Zależności
`tools-window-contract` (bramka GUI), `tools-common-contract`, `safety-broker-contract`, `platform-contract` (`InputPort`, `execute_input`, `KeyChord`), `core-bus-contract`.

## Niezmienniki
- Okno chronione → odmowa przed Brokerem; w porcie (`execute_input`) **przed każdą paczką**: okno z fokusem (klawiatura) / pod punktem (mysz) = cel, proces nie chroniony (PID, obraz, katalog instalacji; nieznany = chroniony), proces nie podniesiony (UIPI). Broker-UI wyskakujący w trakcie pisania nie dostaje ani jednego zdarzenia.
- Paczka = jedno `SendInput` (atomowo; każde wciśnięcie ma puszczenie) → przerwanie nie zostawia wciśniętych klawiszy.
- Fizyczne (niewstrzyknięte) wejście użytkownika: świeże przed startem → `UserActive`; w trakcie → przerwanie przed następną paczką (`Cancelled`, `interrupted_by_user`).
- Skróty systemowe (Win+…, Alt+Tab, Alt/Ctrl+Esc, Ctrl+Alt+Del, kill-switch `Ctrl+Shift+F12`) = odmowa.
- Punkt kliknięcia zawsze w obrębie okna; element musi należeć do tego okna, być widoczny i włączony.
- Limit tempa: odstęp paczek (20 ms, 16 jednostek tekstu), ≤ 5000 znaków, ≤ 60 wywołań/min na sesję.
- Anulowanie przebiegu (kill-switch, „stop”) przerywa przed następną paczką.

## Zdolności / uprawnienia
`gui.control(<aplikacja okna>)`; odwracalność **`no`** (wyższe ryzyko; na L3 poza wskazanymi aplikacjami — pytanie).

## Izolacja
`inproc`, `lazy`; `SendInput` na `spawn_blocking`; hook aktywności na dedykowanym wątku (`platform-windows-gui-impl`).

## Budżet zasobów
RAM ≤ 4 MB; ~800 znaków/s; reakcja na fizyczne wejście ≤ 1 paczka (≤ 20 ms).

## Konfiguracja (klucze TOML)
`[tools.input] max_calls_per_minute = 60`, `max_text_chars = 5000`; tempo w `[platform.gui]` (`InputPacing`).

## Wkład do UI
Kroki w wątku/Replay; „Przejmij / Pauza / Stop” w panelu „Ekran” (F6-08, inna sesja) — fizyczne wejście i tak przerywa.

## Testy akceptacyjne
- `ACC-F6-tools-input-01`: 0 zdarzeń w oknach Alfy/Brokera w 200 losowych scenariuszach (pisanie, skróty, kliknięcia w dowolny punkt, Broker-UI wyskakujący w trakcie, fizyczne wejście) — property, `tests/input.rs`.
- `ACC-F6-tools-input-02`: przerwanie przy fizycznym wejściu (po 2 paczkach), `UserActive` przed startem.
- F6-06 (self-hosted): 0 sukcesów `SendInput` do Broker-UI w 50 próbach (UIPI + strażnik).
- F5-10 (self-hosted): dyktowanie do 5 aplikacji ≥ 95% zgodności.

## Fake
`tools-input-fake`: prawdziwe manifesty i walidacja; testy impl na `FakeDesktop` (wirtualny zegar, skrypty zdarzeń).

## Otwarte pytania
- Helper `uiAccess` dla okien podniesionych (F6-07) — osobny moduł, objęty tym samym strażnikiem.
- Dotyk/pióro (`InjectTouchInput`) — P2.

## Przegląd bezpieczeństwa #2 (2026-10, `docs/reviews/2026-10-security-review-2.md`) — propozycje
- `input_type_text` nie sprawdza, czy element z fokusem to pole hasła (UIA `SetValue` odmawia) — potrzebny odczyt elementu z fokusem (`UiaQuery.focused` albo `UiaPort::focused`; zmiana `platform-contract`/`platform-fake`).
- Strażnik celów liczy obraz procesu okna `GA_ROOT`: wyskakujące okna WebView2 Alfy (lista `<select>`, menu kontekstowe — proces `msedgewebview2.exe`) i okna UWP (`ApplicationFrameHost.exe`) nie są przypisane do właściwego procesu — sprawdzać także `GA_ROOTOWNER` i PID-y procesów WebView2 Alfy (`BrowserProcessId`) w `TargetGuard` przy kompozycji.
- Globalne skróty Alfy (`Ctrl+Alt+Space`) działają mimo strażnika — dodać konfigurowalne skróty Alfy do `system_scope`.
