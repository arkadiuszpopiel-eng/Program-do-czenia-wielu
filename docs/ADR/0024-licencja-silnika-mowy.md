# ADR 0024 — Licencja silnika mowy (następca Pipera na GPL-3.0)

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na podpis właściciela (brak rekomendowanej decyzji; to pytanie do właściciela) |
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
