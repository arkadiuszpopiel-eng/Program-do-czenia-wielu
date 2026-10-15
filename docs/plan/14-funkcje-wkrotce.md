# 14 — Funkcje oznaczone „wkrótce” i funkcje zależne od modułów

Stan na **2026-10-11**, wersja 0.0.33. Zgłoszenie właściciela: „w programie jest dużo funkcji z informacją
«wkrótce» — czy wszystko jest zanotowane?”. Ten dokument jest spisem: każde miejsce w programie, które mówi
„wkrótce” albo „funkcja dostępna po podłączeniu modułu”, ma tu wiersz i zadanie w
[roadmapie](03-roadmapa.md). Test `app-core/tests/soon_inventory.rs` nie przejdzie, jeśli w Ustawieniach
pojawi się strona „wkrótce”, której tu nie ma.

## 1. Strony Ustawień ze znakiem „wkrótce”

Znak „wkrótce” w nawigacji Ustawień dostaje strona, która w `crates/app-core/data/settings-pages.json` ma
`wave` większe niż 1. Po wejściu widać „Ta strona jest jeszcze w przygotowaniu” i listę zapowiedzi.

| Strona (`id`)             | Zapowiedzi na stronie                                 | Co już jest w rdzeniu                                                                                       | Zadanie |
| ------------------------- | ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ------- |
| Logi i prywatność (`logs`) | retencja i redakcja; „co poszło do chmury”            | dziennik NDJSON i audyt są zapisywane; nie ma przeglądarki ani ustawień retencji w oknie                    | C4.2    |

Strona `advanced` działa od serii po 0.0.59 jako „Zaawansowane”: surowy edytor `shared.toml` i nakładki
maszyny (sprawdzanie składni z numerem wiersza, zapis z pełną walidacją rdzenia i przywróceniem pliku
przy odmowie) oraz flaga `experimental.diag_daily` — zadanie C4.4.

Strona `router` działa od wersji 0.0.51 jako „Dobór modelu” (dobór według trudności, szczeble modeli,
Krytyczka, podgląd). Z jej dawnych zapowiedzi zostały w planie tagi prywatności i jurysdykcji w oknie —
zadanie C4.1.

Uwagi do kolejności:

- **C4.2 „co poszło do chmury”** dotyka audytu i tagów prywatności — to obszar Jądra, więc widok może tylko
  czytać; zakres uzgadniam z właścicielem przed pracą (decyzja w tabeli blokad roadmapy).
- **C4.3 włączanie i wyłączanie modułów** nie może obejmować Brokera, watchdoga, audytu ani kill-switcha
  (reguła twarda z `AGENTS.md`).
- **C4.4 edytor TOML** nie może zapisywać kluczy polityki Jądra (rdzeń już je odrzuca — `ConfigError::KernelPolicy`).

## 2. Funkcje, które mówią „dostępna po podłączeniu modułu”

To nie są zapowiedzi. Rdzeń ma dla każdego modułu bezpieczną odpowiedź na wypadek, gdyby moduł nie wystartował:
błąd `unavailable` z nazwą modułu. W zwykłej budowie moduły są podłączone i komunikatu nie widać; pojawia się,
gdy brakuje silnika, modelu albo moduł zgłosił błąd przy starcie.

| Funkcja w programie                                   | Moduł                                             | Kiedy zobaczysz komunikat                                  | Powiązane zadanie          |
| ----------------------------------------------------- | ------------------------------------------------- | ---------------------------------------------------------- | -------------------------- |
| Rozmowa głosowa, mikrofon, test mikrofonu             | `voice-pipeline`, `voice-audio`                   | brak silników mowy albo błąd urządzenia audio              | A.5                        |
| Czytanie na głos                                      | `voice-readaloud`, `voice-tts`                    | brak silnika mowy                                          | A.5, C1.1                  |
| Dyktowanie                                            | `voice-dictation`, `platform-windows-gui`         | brak modelu rozpoznawania mowy                             | A.5                        |
| Słowa wywoławcze                                      | `voice-wake`                                      | brak modelu słów wywoławczych                              | A.5                        |
| Rozpoznawanie głosu właściciela                       | `voice-speaker`                                   | brak modelu głosu                                          | A.5, C3.13                 |
| Model lokalny                                         | `providers-local`                                 | brak silnika albo modelu                                   | A.4                        |
| Wyszukiwanie semantyczne                              | `search`                                          | brak modelu osadzeń — działa wtedy wyszukiwanie po słowach | A.4                        |
| Eksport, import, podgląd paczki `.alfa`, kopie zapasowe | `transfer`                                        | błąd startu modułu                                         | —                          |
| Cofnięcie kroku agentki                               | `undo-journal`                                    | błąd startu modułu                                         | —                          |
| Okno zatwierdzeń, podgląd pulpitu                     | `safety-broker`                                   | Broker niepołączony (tryb bezpieczny)                      | A.6 (Broker poza procesem) |
| Kreator agentek, umiejętności, Ulepszacz, diagnosta   | `agent-builder`, `skills`, `improver`, `diagnostician` | błąd startu modułu                                         | —                          |
| Porządkowanie pamięci                                 | `memory-consolidation`                            | błąd startu modułu                                         | —                          |
| Evale w „Zdrowie systemu”                             | `evals`                                           | błąd startu modułu                                         | C2.1                       |
| Okna wyboru pliku i katalogu, zapis przez okno, terminal | powłoka programu                                  | tylko w testach bez okna (nie w programie)                 | B.5                        |

Wiersze z „—” nie mają otwartego zadania: funkcja działa, a komunikat jest tylko zabezpieczeniem.

## 3. Funkcje zaplanowane, których w programie jeszcze nie widać

Tych miejsc program nie pokazuje wcale (nie ma znaku „wkrótce”), ale są w planie — żeby spis był pełny:

| Funkcja                                                        | Zadanie       | Stan                                      |
| -------------------------------------------------------------- | ------------- | ----------------------------------------- |
| Kamery sieciowe (IP, Wi-Fi)                                    | C3.5          | zgoda właściciela z 2026-10-11; do zrobienia |
| Opis obrazu modelem lokalnym, narzędzie `camera.look`          | C3.6, C3.7    | czeka na pobranie modelu i zdolność Brokera |
| Rozpoznawanie zapisanych osób i pamięć osób, strona „Osoby”    | C3.8–C3.13    | czeka na podpis ADR 0025                  |
| Nowy głos lokalny: poziom naturalny i szybki, cztery głosy agentek | C5.1–C5.7 | wybrane 2026-10-11; czeka na zgodę na pobranie do próby |
| Głos z chmury jako opcja                                       | C1.1a         | po C5; ADR 0026                           |
| Kanwa artefaktów, zadania zaplanowane z budżetem, MCP 2026-07-28 | C2.5–C2.8     | planowane                                 |
| Oznaczanie treści AI (termin prawny 2026-12-02)                | C2.9          | planowane                                 |
| Panel WWW, telefon, Linux                                      | etapy D, E, F, G | czekają na podpis ADR 0017–0023           |

## 4. Jak utrzymać ten spis

- Nowa strona Ustawień z `wave` większym niż 1 → wiersz w sekcji 1 i zadanie w roadmapie (pilnuje test).
- Nowe `AppError::unavailable(…)` w kodzie → wiersz w sekcji 2.
- Strona przestaje być „wkrótce” (`wave` = 1) → wiersz znika stąd, zadanie w roadmapie dostaje „zrobione”.
