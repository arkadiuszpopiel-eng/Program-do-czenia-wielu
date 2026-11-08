# ADR 0017 — Strategia wieloplatformowa: Windows 11 pierwszy, potem panel WWW, Android 15+ i Linux

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na podpis właściciela |
| Data | 2026-10-09 |
| Powiązane | plan v6: `docs/plan/01-wizja-i-zasady.md` (kolejność platform), `02-architektura-docelowa.md` (mapa zdolności, mapa gotowości kodu), `03-roadmapa.md` (etapy A–G); ADR 0001, 0002, 0018 |
| Zmienia | ustalenie planu v5 „cel: tylko Windows 11 x64” (ADR 0001, `AGENTS.md` — sekcja „Stos”) |

## Kontekst

Plan v5 opisuje Alfę jako program na jeden komputer z Windows 11. Właściciel chce tego samego asystenta
na kilku ekranach: na komputerze z Windows, później z Linuksem, w przeglądarce przez Internet i w telefonie
z Androidem. Przegląd kodu z 2026-10-09 (odczyt, bez budowania) pokazał, że rdzeń logiki jest już od systemu
niezależny i CI buduje go także na Ubuntu, ale korzeń kompozycji wpisuje typy Windows wprost w ok. 10 miejscach,
a część kontraktów Jądra ma semantykę Windows wpisaną w typy. Bez decyzji o kolejności i o zasadzie podziału
każda kolejna funkcja Windows utrudniałaby późniejsze przeniesienie.

## Decyzja

| Element | Propozycja |
|---|---|
| Kolejność | 1. **Windows 11 x64** (host i interfejs) → 2. **panel WWW** (pilot w przeglądarce) → 3. **Android 15+** (pilot w telefonie) → 4. **Linux** (drugi system dla hosta i interfejsu) |
| Dlaczego WWW i Android przed Linuksem | korzystają z tego samego fundamentu (host bez okna, protokół zdalny) i nie wymagają przepisania warstwy systemowej; Linux to ok. 30 portów systemowych i najmniej nowego dla użytkownika |
| Kto może zmienić kolejność | właściciel; etapy D–G są rozpisane tak, żeby G (Linux) dało się zacząć zaraz po B |
| Jeden rdzeń | rdzeń nie wie, na jakim systemie działa ani jaki klient z nim rozmawia; wszystko systemowe jest portem w `platform-contract`, wszystko widoczne dla klienta jest poleceniem albo zdarzeniem w `app-api` |
| Nowa platforma | = nowa implementacja portów (`platform-linux-*`, `platform-android-*`), bez zmian w rdzeniu; wybór implementacji w jednym miejscu (ADR 0018) |
| Reguła zależności | `deny.toml` dopuszcza crate `windows` tylko w crate'ach platformy — analogiczna reguła dla każdego systemu (np. `zbus`, `jni` tylko w `platform-linux-*` / `platform-android-*`) |
| Brak zdolności | port, którego system nie ma, zwraca `Unsupported`; interfejs ukrywa funkcję na podstawie mapy zdolności hosta — funkcja jest ukryta, nie zepsuta |
| Pierwszeństwo | etap A (zamknięcie MVP na Windows) przed każdą nową platformą; dokumentacja wszystkich czterech powstaje teraz, żeby decyzje dla Windows nie zamknęły drogi pozostałym |

Mapa zdolności (z `02-architektura-docelowa.md`; stan planowany, nie zmierzony):

| Zdolność | Windows | Linux | Android bez komputera |
|---|---|---|---|
| Rozmowa — model chmurowy | tak | tak | tak |
| Rozmowa — model lokalny | tak | tak | nie |
| Głos (STT/TTS) | tak (WASAPI) | planowane (PipeWire) | planowane (API systemu) |
| Narzędzia: pliki, sieć | tak | tak | nie |
| Narzędzia: powłoka | tak | po analizatorze POSIX | nie |
| Sterowanie komputerem (UIA, wejście, ekran) | tak | badawcze (AT-SPI, portale) | nie |
| Office, rejestr | tak | nie | nie |
| Pamięć i wyszukiwanie | tak | tak | tylko bieżąca rozmowa |

Co zostaje tylko na Windows (do czasu osobnej decyzji):

- siedem crate'ów `platform-windows-*-impl` oraz `voice-audio-impl` (WASAPI) i z nim głos na żywo;
- Jądro poza procesem w obecnej postaci: usługa, potoki z DACL, okno Brokera na podwyższonej integralności,
  dowód fizycznego wejścia, skrót watchdoga;
- launcher, aktualizator, instalator NSIS i proces wydania;
- narzędzia sterowania komputerem: okna, UIA, wejście, ekran, Office, rejestr, OCR, terminal.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Zostać przy samym Windows | sprzeczne z celem „jeden asystent, wiele ekranów”; każda odłożona decyzja o szwach podnosi koszt przeniesienia |
| Linux przed panelem WWW i Androidem | najwięcej pracy „pod spodem”, najmniej nowego dla użytkownika; pilot zdalny nie potrzebuje nowej warstwy systemowej |
| Osobna baza kodu dla każdej platformy | trzy klienty i dwa hosty rozjechałyby się; plan zakłada jeden interfejs Svelte i jeden rdzeń |
| Warunki `cfg(target_os)` rozsiane po modułach zamiast portów | łamie regułę „kod systemowy tylko za portami” i uniemożliwia testy na atrapach |
| Wszystkie platformy równolegle | etap A nie jest zamknięty; nowe platformy powielałyby niesprawdzony kod |

## Konsekwencje

- Potrzebne są cztery szwy opisane w planie v6: fabryka platform i kontrakty Brokera niezależne od systemu
  (ADR 0018), protokół niezależny od transportu (ADR 0019), host bez okna (ADR 0020).
- Zapis w `AGENTS.md` „Cel: Windows 11 x64; kod OS-specyficzny tylko w `platform-windows`” wymaga zmiany —
  zmianę `AGENTS.md` zatwierdza człowiek, osobno od tego ADR.
- Katalog silników i modeli dostaje wymiar system / architektura (zadanie B.6); zmienne środowiskowe wpisane
  na sztywno w binarkach Jądra, watchdoga i aktualizatora przechodzą przez `AppPaths` (B.8, przegląd właściciela).
- Interfejs musi działać na szerokości telefonu (zadanie E.1; dziś minimum okna to 400 × 500 px).
- Kryterium wyjścia etapu G w roadmapie: scenariusz MVP na Ubuntu LTS z kartą NVIDIA; do tego czasu żadna
  funkcja Linuksa ani Androida nie jest uznana za działającą.
- Zasada „kryterium spełnione dopiero na sprzęcie” (ADR 0016) obowiązuje każdą platformę osobno.

## Jak cofnąć

- Kolejność platform jest zapisem planu, nie kodu — zmienia ją właściciel wpisem w roadmapie.
- Rezygnacja z którejkolwiek platformy nie dotyka rdzenia: jej crate'y platformy i opakowanie klienta po prostu
  nie powstają; szwy z etapu B pozostają korzystne dla samego Windows (testy na atrapach, czystszy korzeń kompozycji).
- Do podpisu tego ADR obowiązuje stan z planu v5: jedynym celem jest Windows 11 x64.
