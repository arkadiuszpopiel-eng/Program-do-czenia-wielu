# lib-markdown

Wspólna biblioteka (kategoria `lib-*`, bez logiki modułu): **Markdown z LLM → bezpieczny HTML w Rust**
(ADR 0009, THREAT_MODEL S12). UI (`ui-shell`) tylko wstawia gotowy HTML do DOM; podświetlanie składni
robi leniwie w Web Workerze po zamknięciu bloku kodu.

## Potok
1. `pulldown-cmark` 0.13 — CommonMark + GFM: tabele, listy zadań, przekreślenia; autolinki „gołych”
   adresów (`https://…`, `www.…`) dokłada writer (reguły GFM obcinania interpunkcji).
2. Własny writer (`src/writer/`):
   - **surowy HTML z Markdownu jest escapowany** (widoczny jako tekst, nigdy renderowany),
   - linki tylko `http(s)://host…` i `mailto:`; `javascript:`/`data:`/`vbscript:`/`file:`, adresy
     względne i protokołowo-względne → sam tekst linku (sprawdzamy URL po oczyszczeniu tak jak
     przeglądarka: TAB/LF/CR usuwane, C0 na brzegach, encje zdekodowane przez parser),
   - **obrazy domyślnie wyłączone** (URL obrazka może wynieść dane — prompt injection): zamiast `<img>`
     link „obraz: alt”; flagi `allow_remote_images` (tylko `https://`) i `allow_data_images`
     (tylko `data:image/png|jpeg|webp;base64`, bez SVG),
   - bloki kodu: `<pre data-code="i" data-lines="n"><code data-lang="…">` z escapowaną treścią
     + `CodeBlockMeta { index, lang, lines, text }` dla akcji „kopiuj / zapisz / uruchom”,
   - tabele: wyrównanie jako `data-align` (bez `style` — ścisłe CSP bez inline),
   - listy zadań: `<input type="checkbox" disabled>` (tylko do odczytu).
3. `ammonia` 4.2 z białą listą (`ALLOWED_TAGS`): bez `script/style/iframe/object/embed/form/svg/math`
   (usuwane z treścią), bez atrybutów `on*`, `style`, `id`, `class`; względne URL odrzucane; na każdym
   `<a>`: `rel="noopener noreferrer nofollow"` i `data-external` (UI otwiera link przez powłokę).

## API
- `render(md)`, `render_with(md, RenderOptions)` — cały dokument,
- `render_blocks(md, opts) -> Vec<Block>` — bloki najwyższego poziomu (`id`, `html`, `code`),
- `IncrementalRenderer` — strumień: `push(delta) -> StreamUpdate { closed, open }`, `finish()`.
  Blok jest zamknięty, gdy na **pełnej** linii zaczął się następny (niedokończona linia może jeszcze
  zmienić znaczenie, np. `*` → `**gruby**`); zamknięte bloki nigdy się nie zmieniają, otwarty
  obszar jest renderowany od nowa przy każdej delcie (parsowany jest tylko ogon od otwartego bloku),
- `to_spoken_text(md)` — kanał mówiony (PLAN §6.7): bez znaczników, blok kodu → „(kod na ekranie)”,
  kod w linii > 40 znaków → ta sama fraza, tabela → „(tabela na ekranie)”, link → sam tekst,
  obraz → tekst alternatywny, nagłówek kończony kropką.

Świadome ograniczenie: każdy blok jest renderowany z własnego fragmentu źródła, więc definicje
odnośników `[x]: url` działają tylko w obrębie jednego bloku najwyższego poziomu (dzięki temu
render całości = złożenie bloków ze strumienia). Linki względne (np. do plików) nie są linkami —
podgląd plików (PLAN §14.8) wymaga osobnego schematu, do ustalenia z `ui-shell`.

## Testy
- XSS: 72 wektory (OWASP XSS Filter Evasion Cheat Sheet + Markdown: linki/obrazy w różnych
  kodowaniach, autolinki, tabele, listy zadań, zagnieżdżenia, mXSS) × 2 zestawy opcji
  + 1000 losowych wrogich wejść (proptest); wyrocznia sprawdza znaczniki, atrybuty i schematy URL
  na poziomie wyniku — **0 przejść**,
- własności strumienia (500 przypadków, losowe fragmenty i podziały na delty): zamknięte bloki są
  prefiksem renderu bieżącego bufora, otwarty = reszta, wynik końcowy = `render_with` całości,
- podział na bloki = jeden przebieg parsera po całości (400 przypadków),
- jednostkowe: GFM, escapowanie surowego HTML, metadane kodu, obrazy, schematy URL, tekst mówiony.

## Wydajność (budżet: 100 KB < 20 ms w release)
`cargo test -p lib-markdown --release --test perf -- --ignored --nocapture`

| Pomiar (kontener CI Linux x64, 2026-09-30) | Wynik |
|---|---|
| render 100 KB (102 626 B → 233 382 B HTML), najlepszy z 10 | **13,3–14,6 ms** (3 przebiegi, maszyna współdzielona) |
| strumień 100 KB w deltach po 64 znaki (łącznie, ~1600 kroków) | 44–53 ms |

Główny koszt to parsowanie HTML w `ammonia` (html5ever); dlatego `render_with` sanitizuje całość
jednym przebiegiem (fragmenty bloków są zbalansowane — równość z blokami sprawdza test własności).

## Zależności
`pulldown-cmark` 0.13.4 (MIT, bez domyślnych funkcji), `ammonia` 4.2.0 (MIT OR Apache-2.0),
`serde` (workspace). Crate nie zależy od żadnego modułu.
