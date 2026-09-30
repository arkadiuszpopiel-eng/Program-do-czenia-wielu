# ui-quick — SPEC (szkic v0)

## Cel
Lekkie powierzchnie poza oknem głównym: **Szybkie pytanie** (globalny skrót → okno 640 px z polem i odpowiedzią), **menu zasobnika** (stan, nowa rozmowa, głos wł./wył., nie przeszkadzać, STOP WSZYSTKIEGO, wyjście) i **pigułka głosowa** (nakładka ~220 × 48 px zawsze na wierzchu z awatarem, falą, stanem mikrofonu, Stop/Wycisz) (PLAN §14.8, §14.9).

## Fala i priorytet
F1: Szybkie pytanie + menu zasobnika. F5: pigułka głosowa. P0 (F1), pigułka P1.

## Kontrakt (szkic Rust)
```rust
// ui-quick-contract — SZKIC
pub enum QuickSurface { QuickAsk, TrayMenu, VoicePill }
pub struct TrayState { pub status: TrayStatus /* Idle | Listening | Speaking | Working | Error | MicOpen */, pub dnd: bool, pub voice_on: bool }
pub trait UiQuick: Send + Sync {
    fn show(&self, s: QuickSurface) -> Result<()>;
    fn hide(&self, s: QuickSurface) -> Result<()>;
    fn set_tray(&self, st: TrayState) -> Result<()>;
    fn quick_ask(&self, text: String) -> Result<SessionId>;          // tworzy/dopisuje do sesji „Szybkie pytania"
    fn expand_to_main(&self, session: SessionId) -> Result<()>;      // Enter = otwórz w pełnym oknie
}
```
Zdarzenia: `quick.ask.opened/submitted/expanded`, `tray.menu.action` (new_chat | voice_toggle | dnd | stop_all | quit), `voice_pill.shown/hidden`, `quick.hotkey.conflict`.

## Zależności
`ui-shell-contract` (okna, WebView współdzielone), `ui-kit`, `platform-windows-contract` (skrót globalny, wykrywanie konfliktów, zasobnik), `shell-integration-contract` (ikona zasobnika), `sessions-contract`, `voice-dialog-contract` (F5, stan dla pigułki), `watchdog-contract` (STOP WSZYSTKIEGO — obsługa poza UI).

## Niezmienniki
- Skrót Szybkiego pytania domyślnie `Ctrl+Alt+Space`, zmienialny, z wykrywaniem konfliktów (np. PowerToys) i regułą AltGr.
- „STOP WSZYSTKIEGO" z menu zasobnika trafia do watchdog/brokera, nie przez WebView (< 200 ms, PLAN §8.6).
- Pigułka to minimalna strona bez frameworka (koszt RAM okna mierzony w F0(f)); pokazuje się tylko przy rozmowie głosowej z ukrytym oknem głównym.
- Stan mikrofonu zawsze kolor + ikona + tekst; wskaźnik prywatności w zasobniku, gdy mikrofon otwarty.
- `Esc` zamyka Szybkie pytanie; `Enter` w odpowiedzi otwiera pełne okno z tą samą sesją.
- Okna korzystają ze wspólnego środowiska WebView2 (jeden proces przeglądarki).

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc` + okna WebView2 (Szybkie pytanie) / minimalne okno (pigułka), `on-demand`.

## Budżet zasobów
Otwarcie Szybkiego pytania ≤ 300 ms (ciepłe), pigułka ≤ 150 ms; RAM pigułki — cel z F0(f); menu zasobnika natywne (0 WebView).

## Konfiguracja (klucze TOML)
`[quick] hotkey = "Ctrl+Alt+Space"`, `width = 640`, `session_mode = "single" | "new_each"`; `[tray] close_to_tray = true`, `show_mic_indicator = true`; `[voice_pill] enabled = true`, `position` (per maszyna), `always_on_top = true`.

## Wkład do UI
Makiety 4 (pigułka), 5 (Szybkie pytanie), 20 (menu zasobnika).

## Testy akceptacyjne
- `ACC-F1-ui-quick-01`: skrót → okno ≤ 300 ms; `Esc`/`Enter` zgodnie z opisem (E2E na runnerze Windows).
- `ACC-F1-ui-quick-02`: konflikt skrótu (zarejestrowany wcześniej przez inny proces) → komunikat i propozycja innego skrótu, brak awarii.
- `ACC-F3-ui-quick-03`: STOP WSZYSTKIEGO z zasobnika < 200 ms p95 (z `watchdog`).

## Fake
UI testowane z `platform-windows-fake` (skrót, zasobnik) i `voice-dialog-fake` (stany dla pigułki). W F1 strona Szybkiego pytania działa na `FakeAlfaClient` (Storybook, Playwright), pigułka — w trybie demo bez Tauri.

## Implementacja UI (F1)
- `apps/desktop/ui/quick.html` → `src/quick/` (Svelte, osobny punkt wejścia Vite, własny mały słownik PL/EN): pole, odpowiedź strumieniowana pod spodem (ten sam bufor rAF i `SanitizedHtml`), `Enter` — zapytaj; po odpowiedzi `Enter` w pustym polu — `quick_expand_to_main`; `Esc` — `quick_hide`. Budżet: JS + CSS ≤ 40 KB gzip (sprawdza `scripts/bundle-size.mjs`; obecnie ~27 KB).
- `apps/desktop/ui/pill.html` → `src/pill/` — strona **bez frameworka** (DOM + SVG budowane bez innerHTML): awatar mówiącej agentki, fala głośności ≤ 30 kl./s (pauza, gdy okno ukryte; brak ruchu przy `prefers-reduced-motion`), stan mikrofonu ikoną + tekstem, Stop i Wycisz (`voice_stop_speech`, `voice_set_muted`), dane ze zdarzeń `VoicePill`/`MicLevel`. Budżet: JS + CSS ≤ 8 KB gzip (obecnie ~4 KB).
- Menu zasobnika pozostaje natywne (0 WebView); w Storybooku jest tylko podgląd układu i tekstów (makieta 20).
- Komendy i zdarzenia: `apps/desktop/ui/src/lib/api/COMMANDS.md` (sekcja „Okna").

## Otwarte pytania
- Czy Szybkie pytanie ma własną stałą sesję czy tworzy nową za każdym razem — w F1 ustawienie `quick.session_mode` (domyślnie: jedna sesja „Szybkie pytania"); do potwierdzenia w SPEC v1.
