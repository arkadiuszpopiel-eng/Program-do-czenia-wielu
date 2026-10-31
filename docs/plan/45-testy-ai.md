# System pełnego automatycznego testowania przez AI

Polecenie właściciela z 2026-10-31: „opracuj system pełnego automatycznego testowania przez AI każdej funkcji
obecnej i przyszłej, jak będzie w programie, z pełnymi uprawnieniami dla testów przez AI”.

Ten dokument mówi, jak system działa, co pilnuje funkcji przyszłych, jakie uprawnienia mają testy i sesje AI
oraz co zostało do zrobienia. Uzupełnia [24-samodzielne-testowanie.md](24-samodzielne-testowanie.md) (poziomy
`verify`, testy sprzętowe, instancja testowa) — tamten opisuje, **jak** uruchamiać testy, ten — **czy każda
funkcja ma test** i jak AI dopisuje brakujące.

## 1. Zasada

1. **Każda funkcja programu jest wyliczana z kodu**, nie z pamięci: katalog powstaje przy każdym sprawdzeniu.
   Funkcja dodana jutro trafia do katalogu sama — bez dopisywania jej gdziekolwiek.
2. **Nowa funkcja bez testu = sprawdzenie czerwone** (bramka w `verify szybki` i `verify wersja`). Funkcje bez
   testu sprzed bramki są spisane w długu; dług może tylko maleć.
3. **Przeglądy ogólne** przechodzą po wszystkich pozycjach katalogu danego rodzaju, więc obejmują też funkcje
   przyszłe: nowe polecenie palety, nowa strona albo nowe ustawienie jest sprawdzane od pierwszego dnia.
4. **Sesja AI testuje sama**: bierze kolejkę funkcji bez testu, pisze testy, uruchamia je na atrapie, w prawdziwym
   oknie i na sprzęcie, przycina dług i zapisuje wynik w raporcie. Nie pyta o rzeczy, które może sprawdzić sama
   (reguły z dokumentu 24 §3 obowiązują dalej).

## 2. Katalog funkcji

`node scripts/features.mjs` → [`docs/features/KATALOG.md`](../features/KATALOG.md) (generowany) i
`target/features/katalog.json`. Źródła (`scripts/features/sources.mjs`):

| Rodzaj               | Źródło w kodzie                                                                                 | Liczba (31.10) |
| -------------------- | ----------------------------------------------------------------------------------------------- | -------------: |
| Polecenia rdzenia    | `packages/schemas/ipc.v1.json` — schemat łącza generowany z kontraktu                           |            281 |
| Polecenia palety i skróty | `shortcut-registry.ts` + `EXTRA_COMMANDS` w `state/commands.ts`                            |             62 |
| Strony Ustawień      | `crates/app-core/data/settings-pages.json`                                                      |             38 |
| Ustawienia           | ten sam plik                                                                                    |             98 |
| Narzędzia agentek    | `docs/features/narzedzia.txt` — spis pilnowany testem na rdzeniu złożonym jak w aplikacji       |             81 |
| Zdarzenia rdzenia    | `AlfaEvent` w schemacie łącza                                                                   |             41 |

Spis narzędzi: `crates/app-core/tests/feature_catalog.rs` składa rdzeń (Broker w procesie, dziennik cofania,
atrapy wykonania) i porównuje nazwy narzędzi z plikiem. Nowe narzędzie = test czerwony, dopóki spis nie zostanie
odświeżony (`ALFA_UPDATE_FEATURES=1 cargo test -p app-core --test feature_catalog`) — a wtedy bramka katalogu
wymaga testu tego narzędzia. Narzędzia serwerów MCP i wtyczek użytkownika nie należą do spisu (pochodzą
z konfiguracji użytkownika, nie z kodu Alfy; sprawdza je test `mcp_*` i `plugins_*`).

## 3. Ślady testów

`scripts/features/evidence.mjs` szuka identyfikatora funkcji w kodzie testów sześciu poziomów:

| Poziom          | Gdzie                                                                    |
| --------------- | ------------------------------------------------------------------------ |
| rdzeń           | testy Rust (`crates/*/tests`, `src/**/tests*`, moduły `#[cfg(test)]`)    |
| interfejs       | vitest (`__tests__`, `*.test.ts`)                                        |
| przeglądarka    | Playwright na atrapie (`apps/desktop/ui/e2e`)                            |
| aplikacja       | Playwright w prawdziwym oknie przez CDP (`apps/desktop/ui/e2e-app`)      |
| eval            | zamrożone zestawy ocen modeli (`evals/`)                                 |
| przegląd ogólny | test oznaczony `// katalog-przeglad: <rodzaj>[/<grupa>]` (§4)            |

Nie liczą się **testy-spisy** (zgodność kształtu DTO, sygnatury łącza, zapisane odpowiedzi): wyliczają wszystkie
polecenia, ale nie sprawdzają ich działania. Ślad z ostatniego biegu prawdziwej aplikacji (`ipc-seen.txt`)
trafia do katalogu jako „bieg aplikacji”, ale nie do bramki — zależy od maszyny, a bramka ma dawać ten sam
wynik na laptopie, PC i w CI.

Co zalicza funkcję (wystarczy jeden poziom z listy):

| Rodzaj            | Poziomy zaliczające                                                    | Dlaczego                                           |
| ----------------- | ---------------------------------------------------------------------- | -------------------------------------------------- |
| Polecenia rdzenia | rdzeń, aplikacja                                                       | skutki poza oknem — potrzebny test działania       |
| Narzędzia agentek | rdzeń, eval                                                            | jw.; przegląd ogólny nie wystarcza                 |
| Polecenia palety  | interfejs, przeglądarka, aplikacja, przegląd ogólny                    | przegląd uruchamia każde polecenie                 |
| Strony Ustawień   | przeglądarka, aplikacja, przegląd ogólny                               | przegląd otwiera każdą stronę z axe w dwóch motywach |
| Ustawienia        | każdy                                                                  | przegląd zapisuje i odczytuje każde ustawienie     |
| Zdarzenia rdzenia | rdzeń, interfejs, przeglądarka, aplikacja                              | zdarzenie musi być wysłane albo obsłużone w teście |

Ślad to wystąpienie identyfikatora w teście — dowód, że test **dotyka** funkcji. Jakość testu ocenia przegląd
w świeżym kontekście (ADR 0016), nie bramka. Liczą się też komentarze, więc w pliku testu **nie wymienia się
identyfikatorów funkcji, których ten test nie sprawdza** (opis słowami, np. „wklejanie bez formatu”); test,
który naciska klawisz zamiast podawać identyfikator, nazywa sprawdzaną funkcję w tytule albo komentarzu.

## 4. Przeglądy ogólne — funkcje przyszłe bez dopisywania testów

| Przegląd                                  | Plik                                   | Co sprawdza dla KAŻDEJ pozycji                                                                                                  |
| ----------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Polecenia palety (atrapa)                 | `e2e/z-katalog.spec.ts`                | uruchomienie z palety na świeżym oknie: bez wyjątku strony, bez nieobsłużonego odrzucenia, okno nadal przyjmuje klawiaturę      |
| Strony Ustawień (atrapa)                  | `e2e/a11y-all.spec.ts`                 | otwarcie strony i 0 naruszeń WCAG 2.2 AA (critical/serious) w motywie jasnym i ciemnym                                          |
| Ustawienia (prawdziwy rdzeń)              | `e2e-app/z-katalog.spec.ts`            | zapis innej poprawnej wartości, odczyt, przywrócenie; pominięte ze skutkiem poza programem (autostart, kamera) — z powodem w teście |
| Polecenia odczytu (prawdziwy rdzeń)       | `e2e-app/smoke.spec.ts` („przegląd poleceń odczytu”) | odpowiedź zgodna ze schematem łącza                                                                         |
| Błędy rdzenia (atrapa)                    | `e2e/core-errors.spec.ts`              | rdzeń odrzuca każde polecenie — każda strona i panel pokazuje błąd na miejscu                                                   |
| Spis narzędzi (rdzeń)                     | `crates/app-core/tests/feature_catalog.rs` | nowe narzędzie nie przejdzie niezauważone                                                                                   |

Lista poleceń palety pochodzi z samej palety, ustawień — ze schematu rdzenia, stron — z nawigacji Ustawień.
Testy nie trzymają kopii list, więc nie da się „zapomnieć” dopisać nowej pozycji.

## 5. Bramka i dług

- `node scripts/features.mjs --sprawdz` — etap `katalog` w `verify szybki` i `verify wersja` (kod 1, gdy jest
  nowa funkcja bez testu; komunikat mówi, gdzie dopisać test).
- `docs/features/dlug.json` — funkcje bez testu sprzed bramki (założony 31.10). `--przytnij` usuwa pozycje, które
  mają już test. **Dopisanie funkcji do długu (zgoda na funkcję bez testu) to decyzja właściciela** — skrypt
  tego nie robi, a sesja AI nie dopisuje.
- `--kolejka [N]` — N funkcji bez testu z podpowiedzią, gdzie dopisać test (kolejka pracy sesji testującej).

## 6. Pełne uprawnienia dla testów

### 6.1 Sesja AI (Claude Code) — co robi bez pytania

Na podstawie decyzji właściciela (dokument 24, pamięć projektu) i tego polecenia sesja testująca **sama**:

- uruchamia `verify szybki | wersja | sprzet` (z cichymi testami sprzętowymi), buduje wersję testową, uruchamia
  instancję testową obok Alfy właściciela (własny profil, port CDP, sekrety `Alfa/test/…`);
- steruje oknem instancji testowej przez CDP: klika, pisze, zmienia ustawienia, tworzy i usuwa sesje, pliki,
  zadania, wyzwalacze, kopie — wszystko w profilu tymczasowym;
- pobiera modele i silniki do 5 GB na dysk E: (zgoda stała), uruchamia modele lokalne na karcie;
- testy dotykające pulpitu (schowek, Kosz, skróty, Notatnik, Edge, Menedżer poświadczeń) — gdy właściciel nie
  pracuje przy komputerze albo na jego polecenie; dźwięk — tylko w dzień;
- pisze testy, przycina dług, zapisuje raport z liczbami z `raport.json`.

Granice (zostają mimo „pełnych uprawnień”, bo chronią właściciela, nie ograniczają testów): Alfa właściciela
i jej profil są nietykalne (`Alfa-uruchomiona`, `%LOCALAPPDATA%\Alfa`, klucze w Menedżerze poświadczeń),
nigdy `taskkill /IM alfa-desktop.exe`; wspólne z kontem Windows ustawienia systemu (autostart, skróty globalne
zajęte przez jego Alfę) nie są zmieniane; żadnej wysyłki na zewnątrz (poczta, wiadomości) z kont właściciela;
mosty CLI (`claude`, `codex`) uruchamia tylko człowiek; kod Brokera i polityk — tylko na gałęzi do przeglądu.

### 6.2 Instancja testowa — „Pełna automatyka” bez okna zgód

Instancja testowa używa Brokera **w procesie** (budowa testowa nie ma obok programów Jądra), a ten bez okna
zgód odrzuca każdą prośbę: podniesienie do „Pełnej automatyki” (L4) jest niemożliwe, więc funkcje działające
tylko na L4 (operacje administratora, niepewne polecenia głosowe, pełny dostęp agentek) były nietestowalne.

Rozwiązanie bez obchodzenia zgód (gałąź `do-przegladu-testy-pelny-dostep`, **czeka na przegląd właściciela**):

1. Broker w procesie czyta ten sam plik zapamiętanego poziomu co Broker poza procesem
   (`broker-dev/autonomy-standing.json`, decyzja właściciela 27.10 „Pełna automatyka wybierana raz”) —
   wyrównanie trybów, a nie nowy mechanizm.
2. Skrypt instancji testowej zakłada profil tymczasowy z tym plikiem (`ALFA_TEST_AUTONOMY=L4`) — tak, jakby
   właściciel raz zatwierdził „Pełną automatykę” w tym profilu. Profil właściciela nie jest dotykany.
3. Kod produkcyjny Brokera, okna zgód i dowodu fizycznego wejścia — bez zmian. Prośby, o które Broker pyta także
   na L4 (wysyłka po kontakcie z obcą treścią), dalej są odrzucane w instancji testowej — ich ścieżkę sprawdzają
   testy Brokera na atrapie okna (`safety-broker-*`, `broker-ui-*`).

### 6.3 Czego AI nie testuje samo (trzy powody z dokumentu 24 §3)

Kamera (sprzętowo odłączona), ocena zmysłami właściciela (czy głos się podoba, czy wygląd jest ładny),
decyzje bezpieczeństwa (testy „do decyzji człowieka”).

## 7. Sesja testująca AI — procedura

1. `node scripts/features.mjs --kolejka 20` — co nie ma testu.
2. Dla każdej pozycji: test na właściwym poziomie (podpowiedź w kolejce); funkcja ze skutkiem w rdzeniu — test
   rdzenia **i** scenariusz w prawdziwym oknie (`e2e-app`).
3. Uruchomienie: `verify szybki` (zmienione pakiety), testy przeglądarkowe danego pliku, prawdziwe okno:
   `ALFA_APP_SPEC=<plik> scripts/verify-app.sh --no-build` (osobny katalog wyniku, bez bramki pokrycia).
4. `node scripts/features.mjs --przytnij` → `node scripts/features.mjs` (katalog) → commit z liczbami.
5. Błąd znaleziony testem: poprawka + test regresji, opis w raporcie.

Nowa funkcja (dowolna sesja): bramka sama zatrzyma `verify`, dopóki funkcja nie ma testu.

## 8. Stan wykonania

| Element                                                           | Stan                                                                                                   |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Katalog funkcji (6 rodzajów, 601 pozycji) i ślady testów          | zrobione 31.10                                                                                          |
| Spis narzędzi agentek z rdzenia (81) + test pilnujący             | zrobione 31.10                                                                                          |
| Bramka „nowa funkcja bez testu” w `verify szybki` i `wersja`      | zrobione 31.10                                                                                          |
| Przegląd ogólny poleceń palety                                    | zrobione 31.10: każde polecenie (4 części) zielone na Edge                                              |
| Przegląd ogólny ustawień na prawdziwym rdzeniu                    | zrobione 31.10 (wynik w raporcie 45)                                                                    |
| Instancja testowa na „Pełnej automatyce”                          | gałąź do przeglądu właściciela                                                                          |
| Spłata długu                                                      | w toku — kolejne sesje testujące (`--kolejka`)                                                          |
| Przegląd ogólny zdarzeń i narzędzi                                | do zrobienia: narzędzia — wywołanie każdego na atrapach portów z poprawnymi argumentami ze schematu    |
