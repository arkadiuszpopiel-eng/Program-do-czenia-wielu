# tools-screen — SPEC (v1: narzędzie zaimplementowane, F6)

## Cel
Zrzut okna, monitora albo obszaru **tylko na żądanie agentki** (żadnych zrzutów w tle), zawsze z maskowaniem: okna Alfy/Brokera, aplikacje z deny-listy zrzutów, pola haseł (UIA `IsPassword`). Podstawa trasy wizji (§7.1); OCR i rozpoznawanie elementów (`tools-vision`) budują na nim później.

## Fala i priorytet
F6, P1.

## Kontrakt
```rust
screen_capture { target: window|monitor|region, window?, monitor?, x?, y?, width?, height?, max_side? (64–4096, dom. 1568) }
  → CaptureOutput { width, height, source_x/y/width/height, scale, masked: [{x,y,width,height,reason}], black_frame, png_bytes }
  + images: [ToolImage { media_type: "image/png", data_base64 }]
pub struct ScreenTools; impl ScreenTools { pub fn new(ScreenToolsDeps { desktop, capture: ScreenCapturePort, broker, config, bus }) }
```
Zdarzenie: `tool.screen.capture` (cel, rozmiar, liczba masek, czarna klatka) — **bez pikseli**.

## Zależności
`tools-window-contract` (bramka GUI), `tools-common-contract`, `safety-broker-contract` (`PROVIDER_APPS`), `platform-contract` (`ScreenCapturePort`, `CaptureRequest`, `mask_plan`), `core-bus-contract`.

## Niezmienniki
- Maskowanie w porcie przed skalowaniem i kodowaniem: okna chronione (`TargetGuard`), `DEFAULT_MASKED_APPS` (menedżery haseł, `CredentialUIBroker`, `consent.exe`) + aplikacje dostawców planów (`PROVIDER_APPS`) + konfiguracja, pola haseł; okno, którego pól haseł nie dało się sprawdzić w budżecie czasu (UIA zawieszone) — **maskowane w całości** (fail-closed).
- Zrzut okna chronionego albo aplikacji z deny-listy jako celu = odmowa (nie czarny obraz).
- Wynik niezaufany (taint `Screen` zgłaszany Brokerowi); fakt „dane prywatne” w żądaniu Brokera (trifecta).
- Piksele nigdy nie trafiają do zdarzeń, logów ani na dysk (brak retencji w tym module).
- Czarna klatka (DRM, `WDA_EXCLUDEFROMCAPTURE`) wykrywana i opisana — podpowiedź trasy UIA.
- Limit PNG (6 MiB) → błąd z podpowiedzią mniejszego `max_side`.

## Zdolności / uprawnienia
Okno: `gui.control(<aplikacja>)`; monitor/obszar: `gui.control(desktop.exe)`. Odwracalność `yes`, `mutating = false`.

## Izolacja
`inproc`, `lazy`; GDI na `spawn_blocking`.

## Budżet zasobów
RAM ≤ 24 MB w szczycie (bufor 4K RGBA ≈ 33 MB tylko przy `max_side` ≥ 3840); zrzut monitora FHD ≤ 300 ms + sprawdzanie pól haseł ≤ 4 s.

## Konfiguracja (klucze TOML)
`[tools.screen] max_png_bytes = 6291456`, `masked_apps = ["bank.exe", …]`; `[platform.gui] capture_password_budget_ms = 4000`.

## Wkład do UI
Panel „Ekran” (F6, inna sesja) może pokazać miniaturę z wyniku narzędzia w wątku (obraz w `ToolOutcome.images`, nie w zdarzeniach).

## Testy akceptacyjne
- `ACC-F6-tools-screen-01`: na atrapie 0 pikseli pola hasła po maskowaniu; okna Alfy i aplikacje dostawców zamaskowane; okno niesprawdzone zamaskowane (`platform-fake/tests/desktop.rs`, `tests/screen.rs`).
- F6-05 (VM): 0 naruszeń w 100 scenariuszach (hasła, okna dostawców).

## Fake
`tools-screen-fake`: prawdziwy manifest i walidacja; `FakeDesktop` renderuje okna kolorami (hasła na czerwono) do asercji pikseli.

## Otwarte pytania
- Deny-lista URL w przeglądarkach (tytuł okna ≠ URL) — `tools-browser`.
- Windows.Graphics.Capture dla okien GPU, których `PrintWindow` nie renderuje — po macierzy aplikacji (F6-03).
