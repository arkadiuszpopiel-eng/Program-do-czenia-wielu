# 54 — Jarvis i dalszy rozwój trybów, 8–9.11.2026

Karta [#22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22).
Zlecenie właściciela: rozwijać i testować program w Cloud do rana, do 8:00; Jarvis dostępny
dla każdej agentki. Właściciel potwierdził termin: **9.11.2026 o 08:00**; zachowano interpretację
czasu polskiego: **Europe/Warsaw = 07:00 UTC**. Szczegóły Jarvisa pozostają według roboczego
kontraktu; dalsze wskazówki właściciela mają pierwszeństwo.

## Zakres i kolejność

Aktualizacja po pełnej próbie: UI **856/856**; Rust **3461 zaliczonych / 20 nieudanych / 24 pominięte**
na `9b8b7416`. [Raport katalogu i wyniki](../reviews/2026-11-08-catalog-platform.md) opisują poprawkę
13 przypadków `app-models` (78 testów zaliczonych po zmianie) i pozostałe zadania.

| Kolejność | Zadanie | Stan |
| --- | --- | --- |
| 1 | Regresja Z.8: produkcyjna integracja modelu-sędziego, kontekst ostatniej odpowiedzi | kod i testy w PR #20, przegląd otwarty |
| 2 | Każdy z pięciu trybów: jawny wybór zakresu rozmowa / globalnie, poprawny stan i zgody | implementacja i testy jednostkowe, pełny E2E w toku |
| 3 | Jarvis: ustawienie wspólne i osobne dla Alfy, Bety, Gamy i Delty | implementacja; testy rdzenia i UI |
| 4 | Profil w delegacjach, zadaniach w tle i rozmowie głosowej | testy uzupełniające w toku |
| 5 | Próba na prawdziwym modelu lokalnym, zapis surowych odpowiedzi i ograniczeń | pierwsze przebiegi niezaliczone; dalsza diagnoza |
| 6 | Regresje CI, szerokie testy, dokumentacja, katalog funkcji i draft PR | w toku |
| 7 | Dalsze małe funkcje trybów i Jarvisa, każda z kryterium i testem | po sprawdzeniu bieżącej zmiany |
| 8 | Raport końcowy: commity, wyniki, niezakończone punkty i kolejność dalszej pracy | na zakończenie okna pracy |

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
