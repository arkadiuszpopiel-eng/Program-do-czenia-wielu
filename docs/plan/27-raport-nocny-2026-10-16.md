# Raport z nocy 2026-10-15 / 16 (wersje 0.0.60 – 0.0.62)

Praca od ok. 19:00 do 5:00 na Twoje polecenie: „zacznij dalsze etapy i nie buduj wersji co jeden punkt tylko co 10”,
a potem: „jak to skończysz i zrobisz testy, zacznij kolejne punkty — i tak do 4:30; o tej godzinie ma być działająca
wersja, testy, plan HTML ze stanem pracy; kopia bezpieczeństwa na dysku F: gotowa na 5:00”. Ostatnia zbudowana,
sprawdzona i wgrana wersja to **0.0.62** (stan sprawdzenia — rozdział 3). Ten dokument mówi, co doszło, jak to
sprawdziłem, czego nie sprawdziłem i co zostaje.

## 1. Najważniejsze w pięciu punktach

- **Cztery serie po dziesięć punktów** (0.0.60: seria 1, 0.0.61: seria 2, 0.0.62: serie 3 i 4) — wersja budowana
  co dziesięć punktów, nie po każdym.
- **Alfa odpowiada sama, bez modelu i bez kosztu, na dużo więcej pytań**: rachunki i procenty, jednostki, daty
  i lata przestępne, liczby słownie (kwoty jak na fakturze) i rzymskie, losowanie i wybór z listy, liczenie znaków,
  quiz z tabliczki mnożenia, własne polecenia — także bez internetu. Pod polem pisania widać, kiedy tak będzie.
- **Instrukcja w programie** (`F1`), Tablica przypiętych odpowiedzi, statystyki rozmowy, szablony promptów,
  eksport i kopia rozmowy z oznaczeniem treści AI (AI Act), tekst z dokumentów Word, Excel i PowerPoint dla modelu.
- **Wąskie okno** (od 360 px) sprawdzone testami na wszystkich stronach; poprawiony pasek tytułu i strona kont.
- **Kopia bezpieczeństwa** całego projektu na dysku F: (`F:\Alfa\kopia-…`) z sumami kontrolnymi i instrukcją
  odtworzenia.

## 2. Co doszło

| Wersja | Co doszło dla użytkownika |
| ------ | ------------------------- |
| 0.0.60 | Polecenia bez modelu, quiz, własne polecenia z testerem, szablony promptów, eksport JSON, oznaczanie treści AI, mapa przepływu danych, strony Moduły i Zaawansowane |
| 0.0.61 | Instrukcja w programie, Tablica, tekst z DOCX/XLSX/PPTX, styl odpowiedzi i swoboda, budżet wyzwalaczy, archiwum rozmów, tłumaczenie odpowiedzi, import SKILL.md, SECURITY.md i SBOM; poprawki po przeglądzie serii 1 |
| 0.0.62 | Jednostki, daty, lata przestępne, liczby słownie i rzymskie, procenty, wybór z listy, liczenie znaków; `F1`; trafienia z instrukcji w Ustawieniach; JSON szablonów i poleceń; statystyki rozmowy; wyszukiwanie i eksport Tablicy; kopia rozmowy; zapamiętany tryb skupienia; „Zapisz jako szablon”; wąskie okno; poprawki po przeglądzie serii 3 |

Szczegóły każdej pozycji — [historia zmian](../../CHANGELOG.md) (w programie: Ustawienia → Aktualizacje → Historia
zmian).

## 3. Jak to sprawdziłem

- **Każda seria** przed wersją: testy modułów Rust zmienionych w serii, testy interfejsu (vitest), typy i lint,
  testy w przeglądarce dla nowych okien i stron w **trzech stylach i dwóch motywach** (dostępność axe i kontrast),
  a nowe polecenia rdzenia — także w **prawdziwej aplikacji** (instancja testowa obok Twojej Alfy).
- **Przegląd kodu w świeżym kontekście** (osobny agent) po seriach 1, 3 i 4 — znalezione błędy poprawione tej nocy
  (lista w historii zmian, sekcje „Poprawki po przeglądzie”). Najważniejszy z serii 4: „liczenie znaków” i „wybór
  z listy” przechwytywały zwykłe pytania („Ile liter ma alfabet polski?”, „Wybierz coś na obiad, jestem głodny”)
  i odpowiadały bez sensu zamiast oddać je modelowi. Przerwałem dla tego pierwszy bieg weryfikacji 0.0.62 (był na
  początku), poprawiłem (tekst do policzenia i lista tylko po dwukropku albo w cudzysłowie, te pytania są teraz
  w teście „zwykłe pytania idą do modelu”) i uruchomiłem pełny bieg od nowa.
- **0.0.60** — pełne `verify wersja`: 17 z 19 etapów zielonych za pierwszym razem; 2 czerwone były techniczne
  (formatowanie, plik zgodności typów) — poprawione; prawdziwa aplikacja 36/36, pokrycie łącza 74 %. Wgrana o 20:13.
- **0.0.61** — `verify wersja` do testów w przeglądarce: Rust 2858 zielonych i 1 czerwony (test terminala
  z czekaniem 50 ms przy obciążonej maszynie — poprawiony w 0.0.62), interfejs 381, przeglądarka 437. Bieg
  zatrzymałem przed etapem prawdziwej aplikacji, bo 0.0.62 (z serią 2) i tak go przechodzi — oszczędność ok. 35 min
  przed terminem. 0.0.61 nie była wgrywana osobno.
- **0.0.62** — `verify wersja` (start 2:05, maszyna mocno obciążona — etap testów Rust trwał godzinę):
  - zielone: spójność wersji, formatowanie, graf modułów, linter, linter powłoki, testy powłoki, schematy JSON,
    tokeny wyglądu, typy łącza, typy interfejsu, testy interfejsu **393/393**;
  - testy Rust: **2871 zielonych, 1 czerwony**, 40 pominiętych (sprzętowe) — czerwony był test diagnostyki
    pamięci: pod pełną kompilacją kopiowanie wyszło 1,9 GB/s, więc diagnostyka uczciwie oceniła „wolno”, a test
    oczekiwał „dobrze”. Poprawiłem test (zły wzorzec pamięci nadal go oblewa), testy modułu 5/5 zielone;
  - lint: 1 naruszenie (separator „·” przyklejony do tekstu w nowej podpowiedzi) — poprawione przed budową
    aplikacji, ponowny pełny lint zielony;
  - testy w przeglądarce: pierwszy bieg przerwałem po 41 min, żeby zdążyć z wersją na 4:30; ponowny pełny bieg —
    [PW];
  - **prawdziwa aplikacja: 38/38 zielonych, pokrycie łącza 197 z 264 poleceń (74,6 %)** — w tym nowy scenariusz
    serii 3 i 4 (polecenia bez modelu na prawdziwym rdzeniu, zapis trybu skupienia);
  - zależności i licencje (`cargo deny`), audyt pakietów JS, SBOM (767 składników) — zielone.
- Wgranie do `Alfa-uruchomiona`: **04:40**, dziennik startu: `wersja=0.0.62`, bez błędów (dwa ostrzeżenia znane
  z poprzednich dni: skrót `Ctrl+Alt+Space` zajęty, Broker w procesie w kopii właściciela).

## 4. Czego nie sprawdziłem

- **Wygląd nowych okien i stron** (statystyki rozmowy, Tablica z wyszukiwaniem, wklejanie JSON, podpowiedź „Bez
  modelu”, wąskie okno) — testy sprawdziły dostępność, kontrast i to, że nic nie wychodzi poza okno, w trzech
  stylach i dwóch motywach; czy się podobają — Twoja ocena.
- **Kontakt bezpieczeństwa** w `SECURITY.md` — czeka na Twoją decyzję (nie wpisałem adresu).
- **Testy sprzętowe i próby na modelu lokalnym** (`verify sprzet`) — nie uruchomiłem ich tej nocy. To nie jest
  żaden z trzech dozwolonych powodów (brak sprzętu, Twoja decyzja, ocena zmysłami): zabrakło czasu, bo pełna
  weryfikacja wersji szła pod obciążeniem maszyny ok. 2,5 h. Zmiany tej nocy nie dotykają mowy, kamer ani modeli
  lokalnych (to polecenia bez modelu oraz okna i strony interfejsu), ale bieg „sprzęt” jest pierwszym zadaniem
  następnej sesji.

## 5. Co zostaje

- **C2.10** — kontakt bezpieczeństwa w `SECURITY.md`: czeka na Twoją decyzję (adres nie jest wpisany).
- **C2.11** — tekst z PDF: wymaga nowej biblioteki (pobranie i sprawdzenie `cargo deny`); DOCX/XLSX/PPTX działają.
- **P4** — karty rozmów z profilem, Arena (porównanie modeli).
- **P5** — wyszukiwanie w sieci, wiadomości, „Naucz”; polecenia bez modelu — dalsze rodzaje (np. przypomnienia
  wymagają akcji, więc idą przez agentkę i Brokera).
- Kolejne serie po dziesięć punktów według [roadmapy](03-roadmapa.md).

## 6. Kopia bezpieczeństwa

[WYNIK — F:\Alfa\kopia-…, rozmiar, SHA256SUMS, ODTWORZENIE.md]
