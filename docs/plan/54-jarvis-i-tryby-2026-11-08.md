# 54 — Jarvis i dalszy rozwój trybów, 8–9.11.2026

Karta [#22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22).
Zlecenie właściciela: rozwijać i testować program w Cloud do rana, do 8:00; Jarvis dostępny
dla każdej agentki. Właściciel potwierdził termin: **9.11.2026 o 08:00**; zachowano interpretację
czasu polskiego: **Europe/Warsaw = 07:00 UTC**. Szczegóły Jarvisa pozostają według roboczego
kontraktu; dalsze wskazówki właściciela mają pierwszeństwo.

## Zakres i kolejność

Aktualizacja9.11: pełny lokalny UI **877 zaliczonych / 2 nieudane / 0 pominiętych** na `bd389207`
(UI równoważne `7752ac70`); osobno CI156 **879/879 E2E i573/573 jednostkowych**. Porażki lokalnego
Planu Glass pozostają otwarte. Pełny Rust na `988d13f3`: **3485 zaliczonych / 1 nieudany / 24 pominięte**;
test zegara #34 naprawił pojedynczy przypadek, bez przemianowania pełnej próby na zieloną.
[Raport katalogu i wyniki](../reviews/2026-11-08-catalog-platform.md) opisują poprawkę
13 przypadków `app-models` (78 testów zaliczonych po zmianie). Kolejne
[poprawki fixture'ów #27–30](../reviews/2026-11-08-linux-fixtures.md): 95 zaliczonych / 0 nieudanych /
3 pominięte w zakresach ukierunkowanych; późniejszy pełny Rust opisano wyżej.
[Kompozycja OCR/FFmpeg #33](../reviews/2026-11-08-ocr-composition.md) przeszła81 narzędzi i5 prób
straży na atrapach bez problemów. [Własne persony #35–36](../reviews/2026-11-09-custom-personas.md):
Luna z kreatora i delegacja korzystają z Jarvisa; poprawka odtwarza definicje po restarcie.
Obsada konkretnej rozmowy nadal wymaga ponownego przypisania (#37). Lokalne porażki UI i Windows pozostają otwarte.

| Kolejność | Zadanie | Stan |
| --- | --- | --- |
| 1 | Regresja Z.8: produkcyjna integracja modelu-sędziego, kontekst ostatniej odpowiedzi | kod i testy w PR #20, przegląd otwarty |
| 2 | Każdy z pięciu trybów: jawny wybór zakresu rozmowa / globalnie, poprawny stan i zgody | implementacja; pełny lokalny E2E856/856, odrębne błędy CI otwarte |
| 3 | Jarvis: ustawienie wspólne i osobne dla Alfy, Bety, Gamy i Delty | testy rdzenia/UI; własna Luna dziedziczy ustawienie wspólne, także po odtworzeniu katalogu |
| 4 | Profil w delegacjach, zadaniach w tle i rozmowie głosowej | 6 testów rdzenia i syntetycznego głosu zaliczonych |
| 5 | Próba na prawdziwym modelu lokalnym, zapis surowych odpowiedzi i ograniczeń | po #31:40/40 transport, Jarvis20/20 imion, 0/20 jednego zdania; jakość częściowa |
| 6 | Regresje CI, szerokie testy, dokumentacja, katalog funkcji i draft PR | w toku |
| 7 | Dalsze małe funkcje trybów i Jarvisa, każda z kryterium i testem | po sprawdzeniu bieżącej zmiany |
| 8 | Raport końcowy: commity, wyniki, niezakończone punkty i kolejność dalszej pracy | na zakończenie okna pracy |

Próba bez obciążenia i poprawka form imienia: [raport #31](../reviews/2026-11-08-jarvis-identity.md).
CI149 ma osobne porażki Windows/UI/Linux; szczegóły w [raporcie nocnym](../reviews/2026-11-08-nightly-modes-jarvis.md).

Aktualne ponowienie: [Rust988d13f3](../reviews/2026-11-08-rust-rerun.md) —3485 zaliczonych,
1 nieudany,24 pominięte; pełne wszystkie76integracji. Nowa porażka #34: test kolejki a300ms
okna Cofnij; [poprawka testu zegara](../reviews/2026-11-08-queue-clock.md) ma4/4 i20 powtórzeń pary
bez błędu, bez zmiany300ms. [UI #32](../reviews/2026-11-08-ui-geometry.md) —23 nowych E2E i573 jednostkowe
zaliczone; pełne nowe wyniki podano wyżej. Menu mieści się w małym oknie i po powiększeniu.

## Zachowanie Jarvisa

Opis: [kontrakt profilu](../modules/personas/INTERACTION.md). Panel **Agentki** (`Alt+1`)
pozwala wybrać Standardowy / Jarvis / Jak ustawienie wspólne. Ustawienie wspólne znajduje się
w **Ustawienia → Agentki**. Profil obowiązuje od nowej odpowiedzi lub nowego przebiegu.

Jarvis dodaje krótkie meldunki, instrukcję wykonania zlecenia i sprawdzenia wyniku oraz odpowiedzi
nadające się do odsłuchu. Pozostają imię, role, głos i pamięć persony. Właściciel osobno włącza
mikrofon. Tryb zgód, dostępne narzędzia i limity zadania nadal wyznaczają zakres działania.
Werdykt wewnętrznej Krytyczki zachowuje dotychczasowy format.

## Rozwój każdego trybu

Dalsza regresja: [Plan bez aktywnej rozmowy, karta #25](../reviews/2026-11-08-plan-without-session.md).
Interfejs wymaga otwarcia rozmowy przed Planem; skrót bez rozmowy pomija Plan. Wyniki szerokich
prób i ograniczenie dysku opisano w tym raporcie.

| Tryb | Nowa możliwość w tej serii | Sprawdzenie |
| --- | --- | --- |
| Plan | jawnie pokazany zakres rozmowy; wyjście nadal dotyczy tej rozmowy | test zakresu i dotychczasowe testy menu |
| Ręcznie | wybór tej rozmowy lub wszystkich rozmów | test wyboru zakresu i E2E |
| Akceptuj edycje | ten sam jawny wybór zakresu | test jednostkowy ścieżki prośby |
| Automatycznie | ten sam wybór zakresu; regresje modelu-sędziego w PR #20 | test zakresu i integracja Z.8 |
| Pełny dostęp | ten sam wybór zakresu z dotychczasową zgodą Brokera | test zakresu i kłódki rodzica |

## Zasady dalszej sesji

- Czytać `AGENTS.md`, ten plan i [raport bieżący](../reviews/2026-11-08-nightly-modes-jarvis.md).
- Pracować na `feat/nightly-modes-jarvis`; nie nakładać równoległych zapisów na tę samą gałąź.
- Przy każdym problemie zapisać odtwarzalny przypadek; nie powiększać limitów ani nie wyłączać bramek,
  żeby uzyskać zielony wynik. Pomiary modelu i testy czasowe UI uruchamiać bez konkurujących obciążeń.
- Wyniki Linux/headless, fake audio, Chromium oraz Windows/WebView2 i prawdziwy mikrofon opisywać osobno.
- Zmiany chronionych modułów przygotować jako łatkę do przeglądu człowieka; nie commitować ich automatycznie.
- Publikować sprawdzone przyrosty jako draft PR. Niezależny przegląd i zielone CI pozostają warunkiem scalenia.
- Nie uruchamiać mostów CLI z harmonogramu. Nie pobierać kolejnych wielkich modeli bez potrzeby.
- O 07:00 UTC zakończyć nowe zmiany i przedstawić wynik. Etap J pozostaje otwarty do przeglądu i próby sprzętowej;
  obecna seria robocza nie stanowi wydania, wersja pozostaje 0.0.85.
