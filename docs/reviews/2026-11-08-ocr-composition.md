# OCR i FFmpeg w diagnostyce kompozycji

Karta [#33](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/33).
Baza `d0d4e58c`, Linux Cloud x86_64, Rust 1.94.1.
[Wyniki maszynowe](artifacts/ocr-composition-results.json) zachowują stany obu usterek i klasy
wszystkich końcowych wywołań. Raport autorski, przegląd niezależny pozostaje otwarty.

## Przyczyny i poprawka

`media_convert` miało port `FakeExec`, lecz matcher wymagał nazwy `ffmpeg.exe` na Linux.
Domyślna odpowiedź atrapy nie tworzyła wyniku. Matcher sprawdza teraz dokładną ścieżkę
z `app_agents::ffmpeg_path`. Nadal działa zwykły `FfmpegTranscoder` nad atrapą wykonania;
pusty plik FFmpeg w folderze tymczasowym nie jest uruchamiany.

`vision_ocr` zawsze otrzymywało `WinOcr`, także gdy rig obiecywał wszystkie porty na atrapach.
Dodano opcjonalny `PortOverrides.ocr`, przekazywany przez `MediaPorts::ocr` w AppCore.
Domyślne `None` pozostawia `WinOcr` i limit pikseli. Rig używa istniejącego `FakeOcr`;
test wymaga jednego wywołania z niepustym obrazem i tekstu atrapy w wyniku narzędzia.
Zrzut, maskowanie, uprawnienia i schemat wyniku nadal przechodzą przez `VisionTools`.

Nowa zależność jest wyłącznie testowa (`tools-vision-fake`); Cargo.lock dodaje jeden odnośnik
istniejącego pakietu, bez zmiany wersji. Brak nowych ustawień IPC, typów TS i polityk.
Wzorzec `tool_rig/wzorzec.json` ma niezmieniony blob Git
`069ab69179529484c4ce3b47efa08d400bd3807b`; próby miały `ALFA_TOOL_DIAG_UPDATE=0`.

## Dowody

- Pierwotny raport: `media_convert` = `blad:io`, `vision_ocr` = `blad:unsupported`.
- Po samej korekcie matchera (`3b209fac`): **1 test zaliczony / 1 nieudany**; konwersja już `ok`,
  jedynym problemem diagnostyki pozostaje OCR. Nie zmieniano wzorca ani pomijania narzędzi.
- Po wstrzyknięciu OCR (`e71a0ca5`): **8/8** — Jarvis5, syntetyczny głos1, diagnostyka2.
- Na `015fb5bb`: diagnostyka i przegląd manifestów **4 zaliczone / 0 nieudanych / 1 pominięty**;
  pominięty test wymaga ręcznej zgody właściciela w oknie Brokera i nie został wykonany.
  Pełny `app-agents`: **32 zaliczone / 0 nieudanych / 3 pominięte**. W tym dotychczasowe testy
  prywatności obrazu, ról i niedostępnych portów multimediów.
- Po złożeniu inicjalizacji jednym łańcuchem (`49660826`) ponowna diagnostyka: **2/2**.

Raport końcowy obejmuje **81 narzędzi i 5 prób straży, 0 problemów**. Argumenty i dane wyników
sprawdzono schematami, klasy porównano z niezmienionym wzorcem. To nie dowód działania prawdziwego
Windows.Media.Ocr, kodeka FFmpeg, zgód sprzętowych ani mikrofonu.

Clippy app-agents (`--all-targets`) i app-core (lib, diagnostyka, przegląd narzędzi, zgody,
Jarvis/głos) zaliczone przed ostatnim porządkiem inicjalizacji. Kontrola rozmiaru wykryła najpierw
8002 linii app-core przy limicie8000; uporządkowano powtarzane inicjalizacje i opis już obecny
w rustdoc MediaPorts. Ostatecznie0 naruszeń bez nowych wyjątków. Graf1806 krawędzi,0 naruszeń.
Końcowy Clippy lib/diagnostyki na49660826 i IPC397typów/285komend zaliczone. Dokumentacja
HTML została sprawdzona przed publikacją.

Logi: `/workspace/scratch/ocr-ports-*.log`, `tool-diag-ffmpeg-only.log` oraz raport roboczy
`/workspace/wt-nightly/target/verify/narzedzia/raport.json`. Pełny Rust nowego źródła do ponowienia.
Wersja0.0.85, etap J otwarty; nie jest to odbiór na sprzęcie właściciela.
