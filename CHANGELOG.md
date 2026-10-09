# Dziennik zmian Alfy

Każda seria poprawek podnosi numer wersji i dostaje wpis w tym pliku — po ludzku, od najnowszej. Numer jest w
czterech miejscach naraz: `Cargo.toml` (workspace), `apps/desktop/src-tauri/Cargo.toml`,
`apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/ui/package.json`. Szczegóły techniczne każdej pozycji:
[`docs/STATUS.md`](docs/STATUS.md).

## 0.0.3 — 2026-10-09

Dwa nowe style interfejsu i rozszerzony plan rozwoju (wersja 6) w MD i HTML. Bez zmian w rdzeniu.

### Interfejs

- Ustawienia → Wygląd → **Styl**: Klasyczny, **Szkło** albo **Next**. Każdy styl ma wersję jasną i ciemną — wybiera
  ją „Motyw”.
- Szkło: półprzezroczyste tafle na barwnej poświacie. Bez rozmycia tła — reguła projektu zabrania `backdrop-filter`
  poza paletą poleceń; w oknach Szybkiego pytania i pigułki tafla jest kryjąca, żeby tekst był czytelny nad pulpitem.
- Next: granat i cyjan z prototypu Hub AI, siatka w tle, przycisk główny z gradientem. Wersja jasna jest
  wyprowadzona z tej samej palety (prototyp miał tylko ciemną). Fonty zostają systemowe.

### Plan i dokumentacja

- Plan v6 w `docs/plan/`: stan wykonania, architektura docelowa (host i klienci), roadmapa etapów A–H, katalog
  nowych funkcji, osobne dokumenty dla Windows 11, Linuksa, panelu WWW i Androida 15+, dostęp zdalny
  i bezpieczeństwo, zgodność prawna, przegląd technologii na 2026-10-09 ze źródłami.
- Projekty decyzji ADR 0017–0024 (czekają na podpis właściciela) i szkice specyfikacji dziewięciu nowych modułów.
- Wersja HTML całego planu: `docs/html/index.html` (otwierana dwuklikiem), z paskiem postępu etapów. Buduje ją
  `node scripts/docs-html.mjs`; bramka `scripts/gates.ps1` przebudowuje strony i sprawdza odnośniki.
- `STATUS.md` i `KONTYNUACJA.md` uzupełnione o stan po wersji 0.0.2; przewodnik użytkownika ma aktualny numer wersji.

### Znane ograniczenia

- Żadna funkcja planu v6 nie jest jeszcze zbudowana — to dokumentacja i projekty decyzji.
- Część informacji o technologiach pochodzi ze źródeł wtórnych; w `docs/plan/11-technologie-2026-10.md` każda
  pozycja ma etykietę pewności.

## 0.0.2 — 2026-10-09

Stabilizacja po pierwszym teście na laptopie: skalowanie interfejsu, błędy znalezione w przeglądach rdzenia
i interfejsu, porządek w repozytorium. Bez nowych funkcji.

### Interfejs i skalowanie

- Przy skali powyżej 100 % (każde okno od 1920 px) strona była wyższa niż okno i pole pisania lądowało pod dolną
  krawędzią — naprawione.
- Interfejs sam dopasowuje skalę do szerokości okna (1920 → 108 %, 2560 → 125 %, 3840 → 150 %); kolumna rozmowy rośnie
  z oknem; ustawienia są wyśrodkowane i podzielone na siedem grup.
- Na laptopie (1280 px) prawy panel mieści się obok rozmowy, a lista sesji zwija się do paska ikon; na dużym oknie panel
  jest szerszy i wszystkie karty mieszczą się w jednym wierszu.
- Rozmowa otwiera się na ostatniej wiadomości — wcześniej potrafiła zostać kilkaset pikseli wyżej.
- Okno „Szybkie pytanie” nie ma już uciętej prawej krawędzi; kreator powitalny mieści wszystkie kroki w jednym wierszu.
- Drobne: separator „·” nie przykleja się do słów, baner zerwanego Brokera nie powtarza zdania, licznik katalogu modeli
  nie pokazuje „0 pozycji” w trakcie wczytywania.

### Rozmowa i sesje

- Model pobrany bez silnika daje czytelny komunikat z przyciskiem do „Modele i silniki”; kreator proponuje pakiet razem
  z silnikiem.
- „Edytuj i wyślij ponownie” zachowuje załączniki; nieudany start odpowiedzi nie dubluje wiadomości.
- Kolejka offline nie gubi wiadomości przy błędzie jednej sesji i nie wysyła wiadomości z sesji leżącej w koszu.
- Sesja usunięta poza oknem nie zostaje „aktywna”; błąd w jednym zdarzeniu nie blokuje tury w stanie „pisze”;
  zamknięcie okna nie gubi szkicu.

### Głos

- „Przeczytaj na głos” zawsze odmawiało (aplikacja nie miała dostępu do schowka) — naprawione.
- Czytanie i dyktowanie bez silnika lub modelu mowy mówią, czego brakuje i gdzie to pobrać.
- STOP WSZYSTKIEGO kończy także dyktowanie.

### Narzędzia agentek

- Odczyt pliku wczytuje tylko żądany fragment (duży plik nie zabija procesu); limit zapisu obejmuje plik po dopisaniu.
- Nazwy urządzeń Windows (`nul`, `con`, `com1`…) są odrzucane jako ścieżki.
- Wyjście poleceń PowerShell i `cmd` zachowuje polskie znaki.
- Pobierane strony są dekodowane według kodowania serwera; za długie przekierowanie jest odmową.
- Cofnięcie zmian po restarcie aplikacji usuwa też utworzone katalogi; nieudane „Cofnij” schowka można powtórzyć.

### Modele, kopie i import

- Nieudany start modelu nie zostawia „zajętej” pamięci; duże modele mają dłuższy limit czasu ładowania.
- Kopia zapasowa nie powstaje w wersji nieszyfrowanej, gdy nie da się odczytać hasła.
- Import paczki pokazuje osobno elementy, których nie udało się zapisać.

### Porządek

- Katalog modułów `docs/CATALOG.md`, mapa dokumentów, README dla każdego crate’a, żaden plik Rust ponad 400 linii.
- Budowa lokalna pokazuje w „O programie” datę kompilacji i commit.

### Znane ograniczenia

- Odpowiedź modelu lokalnego i rozmowa głosowa nie były jeszcze sprawdzone na żywo po tych poprawkach.
- Analizator poleceń powłoki nie widzi poleceń ukrytych w cudzysłowie (`cmd /c "…"`) — czeka na decyzję właściciela
  (`docs/reviews/2026-10-08-tools-shell-clipboard-net-review.md`).

## 0.0.1

Pierwszy stan uruchamialny (fale F0–F3 w kodzie), testowany na laptopie 2026-10-07.
