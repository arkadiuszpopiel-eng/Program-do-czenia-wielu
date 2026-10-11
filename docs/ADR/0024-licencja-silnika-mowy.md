# ADR 0024 — Licencja silnika mowy (następca Pipera na GPL-3.0)

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na podpis właściciela; od 2026-10-11 z opcją zalecaną (C z zapasem B) |
| Data | 2026-10-09 |
| Powiązane | plan v6: `docs/plan/04-nowe-funkcje.md` (sprawy do rozstrzygnięcia), `03-roadmapa.md` (C1.1, decyzje blokujące); ADR 0004, 0011; `docs/modules/voice-tts/SPEC.md` |
| Zmienia | potencjalnie ADR 0011 (Piper pl_PL jako zapasowy silnik TTS v0) — zależnie od wybranej opcji |

## Kontekst

ADR 0011 przyjmuje Pocket TTS jako silnik mowy v0, a Piper pl_PL jako zapas; wymaga też głosów „o sprawdzonej
licencji”. Przy przygotowaniu planu v6 sesja ustaliła — **źródło wtórne, do potwierdzenia**:

| Ustalenie | Pewność |
|---|---|
| Oryginalne repozytorium `rhasspy/piper` (licencja MIT) zostało zarchiwizowane w 2025-10 | źródło wtórne, do potwierdzenia |
| Rozwijany następca to `OHF-Voice/piper1-gpl` na licencji GPL-3.0 | źródło wtórne, do potwierdzenia |
| Licencje głosów v0 (Pocket, Piper) nie są jeszcze sprawdzone | stan repozytorium: otwarte pytanie w SPEC `voice-tts` |

Jeśli to się potwierdzi, dalszy rozwój silnika będzie się odbywał na GPL-3.0. Licencja samej Alfy jest
„do ustalenia” (`Cargo.toml`: `UNLICENSED`, crate'y prywatne); roadmapa wymienia ten ADR jako blokujący wybór
głosu (C1.1) i wydanie publiczne. Dziś Piper jest już uruchamiany jako osobny proces (SPEC `voice-tts`).
Sesja AI nie jest prawnikiem — poniżej są opcje techniczne, nie opinia prawna.

## Decyzja

**Decyzja nie została podjęta.** Opcje do wyboru przez właściciela:

| Opcja | Co oznacza technicznie | Zalety | Ryzyka i koszty |
|---|---|---|---|
| A — osobny proces + opinia prawna | następca Pipera uruchamiany jako osobny proces (sidecar), z którym Alfa rozmawia przez potok; dystrybucja i zgodność z GPL-3.0 oceniona przez prawnika | bieżące poprawki i głosy; układ sidecar jest już wzorcem projektu (`voice-tts`, `voice-stt`) | czy osobny proces wystarcza przy wspólnej dystrybucji — to pytanie prawne, nie techniczne; obowiązki GPL wobec samego silnika (źródła, informacja o licencji) |
| B — starsze wydanie na MIT i jego głosy | przypięcie ostatniego wydania oryginalnego Pipera na MIT; tylko głosy o sprawdzonej licencji | brak wątpliwości licencyjnych co do silnika | brak poprawek bezpieczeństwa i rozwoju; z czasem starzejące się zależności; licencje głosów nadal do sprawdzenia |
| C — inny silnik | rezygnacja z Pipera jako zapasu; zapasem zostaje inny silnik o licencji zgodnej z planem wydania | brak zależności od GPL | trzeba znaleźć i zmierzyć polski głos (odsłuch to bramka ludzka); ADR 0011 odnotowuje, że wiele silników nie ma polskiego |

Wspólne dla wszystkich opcji (propozycja, niezależna od wyboru):

| Element | Propozycja |
|---|---|
| Stan do czasu decyzji | Pocket TTS pozostaje silnikiem głównym v0 zgodnie z ADR 0011; wybór głosu w C1.1 czeka na ten ADR |
| Potwierdzenie faktów | przed podpisem sprawdzić u źródła: stan repozytoriów, tekst licencji silnika, licencję każdego rozważanego głosu pl_PL |
| Karty licencji | każdy silnik i głos dostaje w programie kartę: skąd pochodzi, na jakiej licencji, do czego wolno go użyć (pozycja z katalogu funkcji) |
| SBOM | silnik mowy i głosy trafiają do listy składników wydania (C2.10) |
| Konsultacja prawna | przed wydaniem komercyjnym — po stronie właściciela (podział ról w planie v6) |

## Uzupełnienie z 2026-10-11 — opcja zalecana

Analiza silników mowy ([`docs/plan/15-glos-lokalny.md`](../plan/15-glos-lokalny.md)) potwierdziła fakty u źródła
i wskazała inny silnik główny. Właściciel zaakceptował rekomendację; podpis pod tym ADR jest nadal potrzebny
przed wydaniem publicznym.

| Ustalenie | Pewność |
|---|---|
| `OHF-Voice/piper1-gpl` jest na GPL-3.0 i wbudowuje espeak-ng | potwierdzone w repozytorium (2026-10-11) |
| Chatterbox Multilingual V3: kod i wagi na MIT, polski na liście 23 języków | potwierdzone w repozytorium |
| `chatterbox.cpp` (uruchomienie bez Pythona, ggml): MIT | potwierdzone w repozytorium |
| Supertonic 3: kod MIT, wagi OpenRAIL-M; repozytorium zarchiwizowane 2026-09-09 | potwierdzone; treść licencji wag do przeczytania |
| Oficjalny Pocket TTS nie ma języka polskiego (6 języków) | źródło pierwotne i wtórne; wpis „Pocket TTS PL” w kodzie do wyjaśnienia (C5.1) |

**Opcja zalecana: C z zapasem B.** Głosem głównym zostaje Chatterbox Multilingual (MIT) uruchamiany jako osobny
proces; zapasem — przypięte wydanie Pipera na MIT z głosem o sprawdzonej licencji. Następcy Pipera na GPL-3.0
nie dołączamy. Zdanie „Pocket TTS pozostaje silnikiem głównym v0” z tabeli wyżej przestaje obowiązywać po
wykonaniu C5.1 — w praktyce i tak mówi wyłącznie Piper.

Co to zmienia w ryzykach: znika pytanie o GPL przy wspólnej dystrybucji; dochodzą — zależność od projektu
społecznościowego bez numerowanych wydań (przypinamy konkretny commit i sumy SHA-256 wag) oraz znak wodny
w mowie Chatterboxa (decyzja właściciela, zadanie C5.7).

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Nie rozstrzygać i dołączyć następcę Pipera tak jak dziś | ryzyko naruszenia licencji przy wydaniu publicznym; roadmapa wymienia tę decyzję jako blokującą |
| Wbudować kod na GPL-3.0 w proces Alfy (biblioteka) | najmocniejsze powiązanie licencyjne; bez decyzji o licencji całej Alfy — nie do przyjęcia |
| Tylko chmurowy silnik mowy | ADR 0011: profil bez kluczy musi działać lokalnie |
| Zdecydować w sesji AI | to decyzja prawna i biznesowa właściciela; sesja przygotowuje opcje |

## Konsekwencje

- Do czasu decyzji nie należy dodawać nowych głosów Pipera do pakietów modeli ani opierać na nim kryteriów
  jakości głosu.
- Opcja A wymaga osobnego budżetu na opinię prawną i ewentualnie zmian w instalatorze (osobny pakiet silnika,
  informacja o licencji, dostęp do źródeł).
- Opcja B wymaga zapisu w `docs/vendor/` przypiętej wersji i planu, co zrobić, gdy w starym wydaniu pojawi się
  podatność.
- Opcja C wymaga rundy w Voice Lab (odsłuch przez właściciela) i może przesunąć C1.1.
- Każda opcja jest wymianą `-impl` za kontraktem `voice-tts-contract`; `voice-dialog` i reszta potoku głosu
  nie zmieniają się.
- Ten sam przegląd licencji warto wykonać dla pozostałych silników i modeli w pakietach — poza zakresem tego ADR.

## Jak cofnąć

- Wybór silnika zapasowego jest odwracalny na poziomie konfiguracji łańcucha zapasowego w `voice-tts`.
- Przejście między opcjami A, B i C nie zmienia kontraktu; zmienia zawartość pakietu modeli i zapisy licencyjne.
- Jeśli fakty ze źródeł wtórnych się nie potwierdzą, ADR zostaje wycofany bez skutków dla kodu.
