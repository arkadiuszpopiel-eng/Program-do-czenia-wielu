# tools-vision — SPEC (v1: narzędzia zaimplementowane, F6)

## Cel
Trasa „wizja” z PLAN §7.1 dla agentek: **OCR** zrzutu okna/monitora/obszaru albo pliku obrazu (linie ze współrzędnymi ekranu — do kliknięcia przez `tools-input`) oraz **opis obrazu** modelem z obsługą obrazów przez Router (klasa „GUI/wizja”). Budowane na zrzutach `tools-screen`: maskowanie okien Alfy/Brokera, aplikacji z deny-listy i pól haseł **przed** OCR i przed wysłaniem do modelu.

## Fala i priorytet
F6, P1 (OCR), P1 (opis obrazu — zależny od modelu z wizją).

## Kontrakt
```rust
vision_ocr      { source: screen|file, target?: window|monitor|region, window?, monitor?, x?, y?, width?, height?, path?, language? (BCP-47) }
  → OcrOutput { source, path?, width, height, scale, origin_x, origin_y, language, text, lines: [{text,x,y,width,height}], masked, black_frame, truncated }
vision_describe { source, target?, window?, monitor?, x?, y?, width?, height?, path?, question? (≤ 500 zn.) }
  → DescribeOutput { source, path?, masked, model, local, text, truncated }
trait OcrPort { recognize(&OcrRequest{image, language}) -> OcrText{language, lines[{text, words[{text, rect}]}], angle}; languages() }
trait DescribePort { async describe(DescribeRequest{image, media_type, question, privacy, session, max_tokens}, cancel) -> Description{text, model, local} }
trait PrivacyLookup { vision_privacy(session) -> Normal | LocalOnly }   // nieznana sesja → LocalOnly
pub struct VisionTools; VisionTools::new(VisionToolsDeps { desktop, capture, ocr, describer, privacy, files: RangeRead, broker, env, deny, config, bus })
pub struct RouterDescriber; RouterDescriber::new(normal: Router hybrydowy, local: Router lokalny, "auto")
```
Zdarzenia: `tool.vision.ocr` (źródło, wymiary, liczba linii i masek), `tool.vision.describe` (źródło, model, lokalnie, prywatna) — **bez tekstu i pikseli**.

## Zależności
`tools-common-contract`, `tools-screen-contract` (argumenty zrzutu, maski), `tools-window-contract` (bramka GUI, okna chronione), `platform-contract` (`DesktopPort`, `ScreenCapturePort`), `providers-contract` (`ModelProvider`), `safety-broker-contract`, `compliance-contract`, `lib-media` (nagłówki), `core-bus-contract`. Port Windows: `platform-windows-ocr-impl` (`Windows.Media.Ocr`, windows-rs, bez `unsafe`).

## Niezmienniki
- Zrzut tylko przez `ScreenCapturePort` z maskowaniem (jak `tools-screen`: `PROVIDER_APPS`, `masked_apps`, pola haseł, okno niesprawdzone = całe zamaskowane); okno chronione jako cel = odmowa **przed** Brokerem. OCR i model dostają bajt w bajt zamaskowany obraz (test).
- Plik: postać ścieżki i deny-lista (także po dowiązaniach) przed Brokerem; format i wymiary z nagłówka (`lib-media`) **przed** odczytem całości — limit bajtów (20 MiB) i pikseli (40 Mpx, bomba dekompresyjna); port Windows sprawdza wymiary dekodera ponownie przed pikselami.
- Opis: prywatność z katalogu sesji; `Private`/`LocalOnly`/sesja nieznana → wyłącznie Router lokalny, bez niego **odmowa** (obraz nie wychodzi z komputera); do modelu tylko PNG/JPEG/GIF/WebP ≤ 5 MiB; prompt systemowy: treść obrazu to dane, nie polecenia.
- Wynik = treść niezaufana (`untrusted = Screen|File`, taint zgłoszony Brokerowi — przy opisie przed wywołaniem modelu), sekrety redagowane, tekst ≤ 20 000 znaków; tokeny jednorazowe zwalniane zaraz po zrzucie/odczycie.

## Zdolności / uprawnienia
Zrzut okna: `gui.control(<aplikacja>)`, monitor/obszar: `gui.control(desktop.exe)`, plik: `fs.read(plik)` — zawsze z faktem „dane prywatne” (trifecta). Odwracalność `yes`, `mutating = false`. Grupy ról: `vision`, `vision.ocr`, `vision.describe` (wbudowana: Wykonawczyni). Wysłanie obrazu do modelu — ta sama klasa wypływu co rozmowa (Router + tag prywatności), bez osobnego `net.egress` (decyzja do potwierdzenia przez człowieka).

## Izolacja
`inproc`, `lazy`; GDI/WinRT na `spawn_blocking` (WinRT w MTA inicjowanym przez windows-core).

## Budżet zasobów
RAM ≤ 64 MB w szczycie (obraz + bitmapa BGRA 2600 px); OCR monitora FHD ≤ 1 s na baseline (do zmierzenia na runnerze Windows).

## Konfiguracja (klucze TOML)
`[tools.vision] max_file_bytes = 20971520`, `max_pixels = 40000000`, `ocr_max_side = 2600`, `describe_max_side = 1568`, `describe_max_bytes = 5242880`, `describe_max_tokens = 1024`, `output_max_chars = 20000`, `masked_apps = []` (dziś wartości domyślne; odczyt z konfiguracji — razem z `[tools.screen]`).

## Wkład do UI
Brak nowych widoków; wynik w wątku jak każde narzędzie (tekst). Pakiety językowe OCR instaluje użytkownik w Ustawieniach Windows (komunikat narzędzia podaje dostępne języki).

## Testy akceptacyjne
- `ACC-F6-tools-vision-01`: OCR i model dostają zamaskowany obraz (0 pikseli pola hasła, okno Alfy zamaskowane), okno chronione = odmowa przed zrzutem (`tests/vision.rs`).
- `ACC-F6-tools-vision-02`: sesja prywatna/nieznana bez modelu lokalnego → odmowa, Router hybrydowy nigdy nie dostaje żądania (`tests/router.rs`, `app-agents/tests/media.rs`).
- `ACC-F6-tools-vision-03`: deny-lista, bomba dekompresyjna (60 000² px), nie-obraz, za duży plik — bez dekodowania (`tests/vision.rs`); parsery nagłówków — testy właściwości `lib-media`.
- F6-01/F6-05 (VM): OCR na prawdziwym `Windows.Media.Ocr` i macierz aplikacji — self-hosted.

## Fake
`tools-vision-fake`: `FakeTools` (manifesty, walidacja, skrypt), `FakeOcr` (deterministyczny, zapis obrazów), `FakeDescriber` (reguła prywatności), `FakePrivacy`.

## Otwarte pytania
- Tesseract/PaddleOCR jako zapas poza Windows (PLAN §7.2) — po decyzji o modelach OCR.
- Opis obrazu jako osobny token Brokera (`net.egress(dostawca)`) dla tras chmurowych — do decyzji człowieka.

## Przegląd fali 3 (2026-10, `docs/reviews/2026-10-wave3-review.md`)
- **W3-06 (zrobione):** tekst wyniku `vision_ocr` (jedyna część widoczna dla modelu) zawiera każdą linię ze współrzędnymi `[x y w h]` — wcześniej współrzędne były tylko w `data.lines`, więc agentka nie mogła kliknąć znalezionego tekstu (`tests/review.rs`).
- **W3-03 (zrobione w `tools-common`):** plik obrazu na udziale sieciowym (UNC/WebDAV) — odmowa przed dostępem.
