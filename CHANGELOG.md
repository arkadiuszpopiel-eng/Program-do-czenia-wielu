# Dziennik zmian Alfy

Każda seria poprawek podnosi numer wersji i dostaje wpis w tym pliku — po ludzku, od najnowszej. Wpis jest też
źródłem okna „Co nowego” w programie (wstęp i pierwsze zdanie każdej pozycji; sekcje „Testy” i „Przyczyna błędu”
są tam pomijane), więc pierwsze zdanie pozycji ma mówić, co się zmieniło dla użytkownika. Numer jest w
kilku miejscach naraz (workspace Cargo, dwa pliki powłoki Tauri, interfejs i manifesty modułów), dlatego zmienia go
wyłącznie skrypt: `node scripts/set-version.mjs <numer>`; spójność sprawdza `node scripts/set-version.mjs --check`
(bramka i CI). Szczegóły techniczne każdej pozycji:
[`docs/STATUS.md`](docs/STATUS.md).

## 0.0.6 — 2026-10-09

Poprawki po przeglądzie wszystkich zmian od 0.0.1 (trzy niezależne przeglądy w świeżym kontekście).

### Rozmowa i interfejs

- Przejście klawiszem Tab do wiadomości powyżej nie odsyła już listy na dół — fokus zostaje w widoku (także z czytnikiem
  ekranu).
- Lista wyboru: polskie litery wpisywane z AltGr przeskakują do opcji; lista zamyka się, gdy fokus przejdzie gdzie indziej;
  klik w tekst etykiety nie otwiera jej ponownie; przy skali powyżej 100 % w dolnej części okna otwiera się w górę, a przy
  prawej krawędzi równa do prawej. To samo przeliczenie skali dostało menu przycisków.
- Styl „Szkło”: przycisk „nowe wiadomości”, podpowiedzi pola pisania i link „pomiń” mają kryjące tło; lista w palecie
  poleceń nie dostaje już zbędnego cienia.
- Tryb wysokiego kontrastu Windows: suwaki i paski postępu wracają do wyglądu systemowego, żeby tor był widoczny.
- Dwa równoległe wczytania tej samej rozmowy nie gubią już zdarzenia „Stop”; usunięcie aktywnej sesji otwiera następną
  raz, nie dwa.
- Przycisk „Uruchom ponownie” po aktualizacji pyta rdzeń o blokadę (trwająca rozmowa głosowa, zadanie agentki) — nie
  zostaje wyłączony po jej ustaniu ani włączony wbrew stanowi.
- Prawy panel przeciągnięty dokładnie na szerokość domyślną nie przeskakuje na szerokim oknie.

### Narzędzia agentek i rdzeń

- Wyjście PowerShella ucięte limitem w środku znaku albo z pojedynczym obcym bajtem zostaje czytelne — wcześniej całe
  zamieniało się w krzaczki.
- Polecenia PowerShella zaczynające się od `using`, `param` albo `#requires` znów działają (dopisywany przedrostek
  kodowania robił z nich błąd składni).
- Kolejka offline: sesja, której odpowiedź nie może wystartować, nie blokuje pozostałych; jej wiadomość zostaje w kolejce
  i nie jest pokazywana jako wysłana.
- `fs_stat` podaje prawdziwy rozmiar pliku także wtedy, gdy ścieżka różni się wielkością liter i rodzajem ukośnika.
- „Co nowego”: pozycje nie urywają się na dwukropku („Szkło.”), skrót „ok.” nie kończy zdania, za długi wpis kończy się
  dopiskiem zamiast po cichu gubić pozycje.

### Numer wersji i dokumenty

- Podbijanie wersji w czterech plikach zostawiało manifesty modułów przy starym numerze i psuło ok. 20 testów — od 0.0.2.
  Numer zmienia teraz jeden skrypt (`scripts/set-version.mjs`), a bramka i CI sprawdzają spójność.
- `docs/html`: bramka i CI sprawdzają, czy zatwierdzona kopia jest aktualna (wcześniej bramka po cichu ją odświeżała);
  sprawdzane są też kotwice, a wiersz roadmapy z nieznanym stanem jest błędem. Poprawione podwójne kodowanie znaków
  w tytułach stron.
- `gates.ps1 -Test a,b` działa zgodnie z opisem; znacznik budowy w „O programie” nie zostaje stary po `git pull`.

### Testy

- Kontrast tekstu jest mierzony po złożeniu warstw (półprzezroczyste tła), we wszystkich stylach i motywach — test
  dostępności nie oceniał tekstu nad gradientem. Miernik ma własny test, który musi odrzucić nieczytelny napis.
- Testy regresji dla każdej poprawki powyżej; test fokusu sprawdzony także bez poprawki (jest wtedy czerwony).
- Wzmocnione testy, które przechodziły także bez poprawek (nazwy urządzeń przy odczycie, zmienne środowiskowe spoza
  Unicode, „rozmowa otwiera się na dole”, powtórzone zdanie w banerze Brokera, kryjące tło okna Szybkiego pytania).

## 0.0.5 — 2026-10-09

Okno „Co nowego” pokazuje krótką listę poprawek.

### Interfejs

- Po zmianie wersji okno „Co nowego” wypisuje skrót zmian tej wersji. Wcześniej, gdy wersja nie miała dołączonych
  notatek (każda budowa lokalna), było tam tylko „Zaktualizowano Alfę do wersji…”.
- Skrót powstaje z tego dziennika zmian, wbudowanego w program: zdanie wstępu i pierwsze zdanie każdej pozycji.

### Testy

- Test pilnuje, że uruchamiana wersja ma w dzienniku wpis z listą zmian — podbicie numeru bez opisu nie przejdzie.

## 0.0.4 — 2026-10-09

Poprawka po zgłoszeniu właściciela: nieczytelne i systemowe listy wyboru.

### Interfejs

- **Listy wyboru są teraz rysowane przez Alfę**, nie przez system: własna lista opcji w stylu interfejsu, ze znaczkiem
  przy wybranej pozycji, obsługą klawiatury (strzałki, Home/End, Enter, Esc, pierwsze litery) i czytnika ekranu.
  W wersji 0.0.3 rozwinięta lista była systemowa, a w stylu „Szkło” miała jasny tekst na białym tle.
- To samo dla pozostałych kontrolek rysowanych dotąd przez system: paski postępu, suwaki, pola zaznaczenia, przycisk
  wyboru pliku i ramka grupy pól biorą kolory i kształt ze stylu interfejsu.
- Paski przewijania bez systemowych strzałek: sam zaokrąglony uchwyt w kolorach stylu.
- Ostatnia natywna lista (wybór przebiegu w odtwarzaniu pracy agentki) zastąpiona tym samym komponentem.

### Testy

- Lista wyboru jest sprawdzana **otwarta** w każdym stylu i motywie (6 kombinacji): tło kryjące, kontrast opcji
  co najmniej 4,5:1, obsługa klawiaturą, brak natywnego `<select>` na stronie.
- Ten sam test w prawdziwej aplikacji (WebView2), ze zrzutem każdego stylu.
- `apps/desktop/ui/scripts/shots-open.mjs`: zrzuty kontrolek w stanie otwartym — uzupełnia zrzuty widoków.

### Przyczyna błędu

Style z 0.0.3 były oglądane tylko na zrzutach w stanie spoczynkowym; listy nikt nie rozwinął. Testy dostępności nie
widzą listy systemowej, bo system rysuje ją poza stroną.

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
