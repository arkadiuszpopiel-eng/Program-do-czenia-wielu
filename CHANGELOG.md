# Dziennik zmian Alfy

Każda seria poprawek podnosi numer wersji i dostaje wpis w tym pliku — po ludzku, od najnowszej. Wpis jest też
źródłem okna „Co nowego” w programie (wstęp i pierwsze zdanie każdej pozycji; sekcje „Testy” i „Przyczyna błędu”
są tam pomijane), więc pierwsze zdanie pozycji ma mówić, co się zmieniło dla użytkownika. Numer jest w
kilku miejscach naraz (workspace Cargo, dwa pliki powłoki Tauri, interfejs i manifesty modułów), dlatego zmienia go
wyłącznie skrypt: `node scripts/set-version.mjs <numer>`; spójność sprawdza `node scripts/set-version.mjs --check`
(bramka i CI). Szczegóły techniczne każdej pozycji:
[`docs/STATUS.md`](docs/STATUS.md).

Waga pozycji: `[krytyczne]` (czerwone — bezpieczeństwo, prywatność, utrata danych, funkcja nie działała wcale),
`[poważne]` (pomarańczowe — istotny błąd albo zabezpieczenie), bez znacznika — zwykła zmiana (zielone). Znacznik stoi
na początku pozycji; program pokazuje go w Ustawienia → Aktualizacje → Historia zmian.

## 0.0.79 — 2026-10-31

Alfa ma spis każdej swojej funkcji i pilnuje, żeby żadna — obecna ani przyszła — nie weszła bez testu, który
AI uruchamia samodzielnie. Narzędzia plikowe podają modelowi krótsze ścieżki.

### Nowe

- Katalog funkcji programu: 601 funkcji (polecenia, skróty i akcje palety, strony i ustawienia, narzędzia
  agentek, zdarzenia) zbierany z kodu przy każdym sprawdzeniu wersji, z informacją, czym każda jest przetestowana
  (`docs/features/KATALOG.md`). Nowa funkcja bez testu zatrzymuje sprawdzenie wersji.
- Przeglądy ogólne: każde polecenie palety jest uruchamiane w oknie testowym, a każde ustawienie zmieniane
  i przywracane w prawdziwej aplikacji — także te, które dopiero zostaną dodane.

### Poprawki

- Narzędzia plikowe podają modelowi pliki z katalogu roboczego ścieżką względną (lista, odczyt, wyszukiwanie,
  zapis, przeniesienie, kopia, Kosz) — model lokalny przepisywał długie ścieżki z literówkami („Tepm”).
  Zatwierdzenia i historia cofania nadal pokazują pełną ścieżkę.

### Testy

- Spłata długu katalogu: testy rdzenia dla poleceń bez testu, narzędzia `system_software` i `system_diagnostics`,
  zdarzenia stanu okna (obsada, koszty, stan systemu, Broker, raport Marszałka), klawisze pola pisania.

## 0.0.78 — 2026-10-30

Kanwa pamięta wersje artefaktu i pokazuje, co się zmieniło. Model lokalny lepiej radzi sobie z narzędziami —
pierwszy pomiar na prawdziwym Bieliku 4.5B wykazał kilka błędów, które ta wersja poprawia.

### Nowe

- Kanwa pokazuje historię wersji artefaktu: gdy Alfa poprawia ten sam dokument, kod albo stronę, przełącznik
  „Wersja N z M” pozwala wrócić do wcześniejszej, a karta „Zmiany” pokazuje wiersze dodane i usunięte względem
  poprzedniej wersji.

### Poprawki

- [poważne] Model lokalny z narzędziami: wywołanie narzędzia jest teraz wyborem na początku odpowiedzi — wcześniej
  Bielik wpisywał je jako zwykły tekst i nic się nie wykonywało (w pomiarze żadne z 35 zadań nie przeszło).
- [poważne] Samoweryfikacja kończy się odpowiedzią dla Ciebie, a nie tekstem „WERYFIKACJA: OK”; wywołanie
  narzędzia doklejone do werdyktu nie zapętla już sprawdzania.
- [poważne] Polecenia powłoki działają na komputerze bez PowerShell 7 — Alfa używa wtedy Windows PowerShell
  zamiast kończyć każde polecenie błędem „nie znaleziono ścieżki”.
- Agentki podają pliki w katalogu roboczym ścieżką względną — lekki model przepisujący długą ścieżkę robił
  literówki i zapisywał pliki w złym miejscu.
- Powtórzone wywołanie z tymi samymi argumentami dostaje podpowiedź, żeby zakończyć, zamiast kręcić się aż do
  wykrycia pętli.
- Model lokalny wywołuje narzędzia z niską temperaturą (0,2), gdy nie ustawisz innej — mniej przekręconych nazw
  plików i stabilniejsze wyniki.
- Tekst przy wywołaniu narzędzia zapowiada, co agentka robi („Odczytuję plik…”), zamiast od razu ogłaszać wynik.
- Wybrana na stałe karta graficzna (CUDA/Vulkan) działa także bez profilu urządzenia — wcześniej serwer modelu
  liczył wtedy na procesorze.

### Testy

- Historia wersji kanwy: testy jednostkowe porównania wierszy i e2e we wszystkich stylach i motywach (axe,
  kontrast). Pomiar F3 na Bieliku 4.5B (llama-server b11559, CUDA): przed poprawkami 0/35, po nich 10–12/35;
  test wykrywania powłoki i podpowiedzi przy powtórzonym wywołaniu.

## 0.0.77 — 2026-10-29

Alfa dla dzieci krok dalej: styl „Junior”, czytane zdanie podświetlone w rytm głosu i uproszczony ekran.
Do tego panele schowane od startu i porządki pod przyszły dostęp zdalny i inne systemy.

### Nowe

- Styl „Junior” (Ustawienia → Wygląd): ciepłe kolory, okrągłe kształty i cały interfejs o 15 % większy. Po
  wybraniu Alfa proponuje wolniejszą mowę (85 %), dźwięki sygnałowe i uproszczony ekran — każdą z tych rzeczy
  zmienisz potem w Ustawieniach. Okienko Szybkiego pytania i pigułka głosowa dostają te same kolory.
- „Przeczytaj na głos” podświetla zdanie, które właśnie brzmi; w rozmowie głosowej podświetlenie idzie za
  odpowiedzią zdanie po zdaniu i znika, gdy mowa się kończy albo wyłączysz mikrofon.
- „Uproszczony ekran” (Ustawienia → Wygląd): mniej przycisków — bez przełączników paneli, Pipeline'ów, wyboru
  agentki i rzadszych akcji wiadomości. Wskaźniki mikrofonu, kamery, sterowania i trybu pracy zostają zawsze.

### Zmiany

- Panele boczne startują schowane (decyzja właściciela 29.10); znacznik przy krawędzi i Ctrl+B wysuwają je jak
  dotąd.
- Katalog silników zna system i procesor każdej binarki — pokazuje tylko to, co działa na tym komputerze
  (przygotowanie pod Linuksa i ARM).
- Typy danych interfejsu pochodzą już wyłącznie z rdzenia (koniec ręcznych kopii), a polecenia wymagające tego
  komputera (okna systemu, ścieżki, schowek) są oznaczone pod przyszły dostęp z telefonu.

### Poprawki

- [poważne] Biblioteka Markdown podniesiona do 0.13.5 (ostrzeżenie bezpieczeństwa dotyczące linków wiki).
- Nieudane logowanie mostu CLI pokazuje jeden komunikat zamiast stosu jednakowych.
- Logowanie mostu poza Windows nie wskazuje już powłoki `cmd`, której tam nie ma.

### Testy

- Nowe testy: podświetlenie w rozmowie głosowej (Rust i przeglądarka), uproszczony ekran, Junior w oknach
  pobocznych i w prawdziwym oknie (odmowa błędnych wartości, wybór i przywrócenie ustawień), wymiar system /
  architektura katalogu, tabela poleceń `local_only`, strażnik generatora typów.

## 0.0.76 — 2026-10-29

Pulpit główny dopracowany według uwag właściciela z 29.10: wszystkie panele chowają się w całości i płynnie
wysuwają, przełączanie trybu pracy naprawdę działa i widać je od razu, a ciemna kapsuła stanu przeszła z belki do
paska pisania.

### Nowe

- Panele Sesje i prawy chowają się w całości — przyciskiem w nagłówku panelu, Ctrl+B albo Ctrl+\ — bez paska
  ikon. Przy krawędzi okna świeci cienki znacznik; najechanie wysuwa panel nad rozmowę, a kliknięcie znacznika albo
  „Przypnij panel” dokuje go z powrotem.
- Menu Ustawień też się chowa (przycisk obok tytułu albo Ctrl+B) i wysuwa z prawej krawędzi; wybór zostaje
  zapamiętany.
- Pasek pisania pokazuje stan Alfy: profil modeli z kropką połączenia, tryb pracy i koszt sesji. Każdy żeton
  otwiera swoje okienko nad paskiem.
- Gdy włączona jest Pełna automatyka, w belce świeci bursztynowy znak „Pełna automatyka” — także w Ustawieniach;
  kliknięcie otwiera menu trybów.

### Zmiany

- Belka nie ma już ciemnej kapsuły stanu; zostały w niej wskaźniki na żywo (sterowanie, mikrofon) i przełączniki
  paneli.
- Panele wjeżdżają i wyjeżdżają płynnie (przy ograniczonym ruchu albo wyłączonych animacjach — od razu).
- Pasek pisania ma zawsze dwa wiersze: tekst na całą szerokość, pod nim spinacz, wybory, stan, mikrofon i
  „Wyślij”. W wąskim polu (obok obu paneli) żetony Pipeline'ów, profilu i trybu zwijają się do ikon, a na
  telefonie przechodzą do własnego wiersza.
- Okienka przy pasku pisania otwierają się w górę, z łagodnym wejściem.
- Znacznik schowanego panelu w spoczynku to sama świecąca kreska — strzałka pojawia się dopiero po najechaniu.
- Bez otwartej rozmowy panel prawy startuje schowany — ekran startowy nie ma pustej kolumny.
- W średnim oknie (ok. 1280 px) pokazanie Sesji chowa panel prawy za znacznik, więc przycisk w belce zawsze coś
  robi.

### Poprawki

- [poważne] Zmiana trybu pracy nie działała: menu po przełączeniu nadal zaznaczało poprzedni tryb, choć komunikat
  mówił o zmianie. Teraz menu czyta tryb z rdzenia, po zmianie sprawdza go jeszcze raz i mówi wprost, gdy zmiana
  się nie udała; rozmowa z własnym trybem ma o tym notkę w menu.
- Profil modeli z paska pisania i z okienka stanu to jedno ustawienie (wcześniej dwa niezależne wybory, które
  mogły się różnić).
- Znacznik menu Ustawień nie nachodzi na wysuniętą listę stron.

### Testy

- Nowy test przeglądarki `panels-slide`: chowanie, wysuwanie i przypinanie paneli w trzech stylach i dwóch
  motywach, średnie okno, płynność przejścia, tryb pracy bez otwartej rozmowy, znak Pełnej automatyki (z axe).
- Testy belki i paska pisania przepisane pod stan w pasku pisania (profil, tryb i koszt od 360 do 1920 px bez
  nachodzenia, „Wyślij” zawsze przy prawej krawędzi); testy jednostkowe układu paneli i zapamiętania menu
  Ustawień.
- Test prawdziwego okna: tryb zaznaczony w menu zgadza się z trybem w rdzeniu, Sesje chowane przyciskiem i
  przypinane, obniżenie trybu z paska pisania dociera do rdzenia (`zz-mode`).
- Dwa testy przeglądu klikały pierwszą wiadomość, którą wirtualna lista po wczytaniu odmontowuje — teraz
  ostatnią, widoczną.

### Przyczyna błędu

- Interfejs znał tylko tryb otwartej rozmowy. Bez rozmowy menu zawsze pokazywało domyślne „Automatycznie”, a
  zmiany trybu ogólnego (zapisywanej w rdzeniu) nigdzie nie odczytywał — komunikat mówił o zmianie, której widok
  nie pokazywał.

## 0.0.75 — 2026-10-28

Porządki interfejsu według uwag właściciela z 27.10: tryb pracy wybierany prosto z belki, okno Brokera bez
odliczania, symetryczna belka tytułu i wyrównany pasek pisania, panele boczne chowane ze świecącymi znacznikami,
mówiące awatary z literami wylatującymi z ust i uporządkowane okno Pipeline'ów.

### Nowe

- Tryb pracy wybierasz z prostej listy w belce: Tylko podgląd, Pytaj o wszystko, Automatycznie, Pełna
  automatyka. Podniesienie zatwierdzasz raz w oknie Brokera, a wybrany tryb zostaje po restarcie; w każdym trybie
  zostają kill-switch Ctrl+Shift+F12, dziennik i pytanie przed wysyłką po obcej treści albo po poleceniu obcym lub
  niepewnie rozpoznanym głosem.
- Panele boczne mogą chować się same (Ustawienia → Wygląd → „Panele chowają się same” albo polecenie „Chowaj
  panele boczne automatycznie” w palecie Ctrl+K). Przy krawędzi okna świeci znacznik; po najechaniu panel wysuwa
  się nad rozmowę, a po zjechaniu myszą, kliknięciu obok albo Esc chowa się. „Przypnij panel” przywraca stały
  układ.
- Ręcznie schowany panel też zostawia znacznik przy krawędzi i wysuwa się po najechaniu.
- Gdy agentka pisze odpowiedź, jej awatar mówi, a z ust wylatują litery z właśnie pisanego tekstu (przy
  wyłączonych animacjach i ograniczonym ruchu — bez liter).
- Okienko stanu w belce działa na miejscu: profil modeli (Lokalny, Hybryda, Chmura) zmieniasz od razu, a „Zmień
  tryb pracy” otwiera menu trybów zamiast przenosić do Ustawień.
- Polecenie palety Ctrl+K „Tryb pracy” też otwiera menu trybów w belce (w Ustawieniach — stronę Uprawnienia).
- Instrukcja w programie (F1) opisuje belkę w trzech strefach, menu „Tryb pracy”, panele chowane i okno
  Pipeline'ów.
- Plan Alfy dla dzieci (etap K): nauka i zabawa głosem, styl „Junior”, limity czasu i podgląd rodzica z telefonu —
  dokument `docs/plan/41`, z pytaniami do właściciela.

### Zmiany

- Okno Brokera przy zmianie trybu pracy nie odlicza już czasu i nie pyta dwa razy; pokazuje nazwy trybów jak w
  menu i przycisk „Włącz: …”.
- Belka tytułu ma trzy równe strefy (marka i ścieżka, obsada na środku, stan i przyciski okna) i jedną wysokość
  wszystkich kontrolek; w wąskim oknie strefy zbliżają się zamiast nachodzić. Nazwa trybu i profilu są widoczne
  od 1280 px (np. na laptopie 1366 px); skrót „L3” zostaje w węższym oknie albo przy kapsule sterowania ekranem.
- Pasek pisania: przyciski i wybory w jednym rzędzie z równymi odstępami i wysokościami; gdy pole rozmowy jest
  wąskie, tekst ma cały górny wiersz, a przyciski schodzą do dolnego („Wyślij” zawsze po prawej).
- Podpowiedź pod polem pisania łamie się całymi fragmentami („Shift+Enter — nowa linia” nie rozpada się na dwa
  wiersze).
- Okno Pipeline'ów: krótszy opis po polsku, „×” do zamknięcia jak w innych oknach, etapy połączone strzałkami
  przepływu, jedno pole wyboru modelu (Auto, modele, „Inny…”), przyciski „+ zadanie” i „+ wynik poprzedniego”
  oraz pasek „Uruchom w rozmowie” na całą szerokość.

### Poprawki

- W oknie szerokim na 1000 px kapsuła sterowania ekranem nie wchodzi już na obsadę agentek; przy 360 px
  belka i pasek pisania nie wystają poza okno.
- W stylu klasycznym numer aktywnego etapu pipeline'u i pasek LED są znowu widoczne (styl nie ma koloru
  akcentu).
- Kliknięcie obsady na środku belki nie przesuwa już okna (obsada była w obszarze przeciągania).
- „Kontynuuj” zaraz po zakończeniu odpowiedzi nie kończy się już fałszywym „Zadanie już pracuje”: koniec tury
  był ogłaszany chwilę przed zwolnieniem sesji; kontynuacja czeka teraz do pół sekundy na to zwolnienie.
- Okno uruchamia się lżej: pakiet startowy zmalał ze 151 do 125 KB. Teksty okien otwieranych później (Hub,
  modele, głos, wtyczki, MCP, diagnostyka i inne) są osobną częścią słownika pobieraną równolegle ze startem,
  a znaczniki chowanych paneli oraz wskaźniki sterowania ekranem i kamery — osobnymi modułami.
- Wskaźniki „Zatrzymaj” (sterowanie ekranem) i kamery pobierają się ponownie po błędzie, a od razu, gdy agentka
  już steruje ekranem albo kamera działa.
- „Przypnij panel” nie zostawia wysuniętego panelu, a Esc nie przepada, gdy panel już się schował.
- Klawiatura: Enter na znaczniku schowanego panelu przenosi fokus do panelu, a po schowaniu wraca na znacznik;
  wybór profilu w okienku stanu działa strzałkami, z jednym przystankiem Tab.
- „Zmień tryb pracy” w bardzo wąskim oknie (bez przycisku trybu w belce) otwiera stronę Uprawnienia.
- Menu „Tryb pracy” w stylu Szkło ma jednolite tło (lista trybów nie odcina się prostokątem innego koloru).
- Bez kluczy API belka i okienko stanu pokazują profil „Lokalny” — ten, który naprawdę działa (jak pasek
  pisania), zamiast zapisanej „Hybrydy” z wyglądem wyłączonego przycisku; wybrany profil ma obwódkę widoczną
  także w jasnym stylu Szkło, a opis połączenia mówi, że modele w chmurze wymagają klucza.
- Pipeline'y: wybór „Inny…” w polu modelu nie przeskakuje na inny etap.

### Testy

- Nowe testy przeglądarkowe: panele chowane we wszystkich stylach i motywach, polecenie palety, menu trybu,
  okienko stanu, litery z ust w pisanej odpowiedzi, klawiatura znaczników, pasek pisania w dwóch wierszach,
  polecenie palety „Tryb pracy”; testy jednostkowe układu paneli i test pilnujący, że kod startowy nie używa
  tekstów z drugiej części słownika.

## 0.0.74 — 2026-10-27

Alfa łączy się z silnikiem gier właściciela NextGO.Gray: agentki uruchamiają jego serwer, budują i zmieniają
świat, oglądają scenę i zrzuty, czytają dokumentację silnika — bez zmian w katalogu silnika.

### Nowe

- Katalog silnika ustawia się w Ustawienia → Zaawansowane → „Katalog silnika gier NextGO.Gray” (pełna ścieżka do
  zbudowanego silnika); puste pole wyłącza narzędzia silnika.
- 15 narzędzi agentek (grupa „engine”): stan instalacji, dokumentacja, start i zatrzymanie serwera, odczyt sceny,
  transakcje z cofaniem, krok symulacji, strumieniowanie świata, raycast, wysokość terenu, zaznaczenie, podmiana
  shadera, zrzut okna i podgląd PNG. Każde wywołanie przechodzi przez Brokera; zmiany świata są akcjami
  zmieniającymi stan, a ponowienie tego samego wywołania nie dubluje zmiany.
- Serwer silnika działa tylko na pętli zwrotnej, z losowym portem, bez kluczy w środowisku i kończy się z
  kill-switchem; zrzuty i podglądy trafiają do danych Alfy, nie do katalogu silnika.

### Poprawki

- Przycisk „Zatrzymaj” sterowania ekranem w średnio szerokim oknie ma znowu ikonę i nazwę dla czytników ekranu
  (był pusty i za mały do kliknięcia).
- Kapsuła „Delta steruje ekranem” w oknie do 1700 px szerokości nie wchodzi już pod obsadę agentek na środku
  belki — pokazuje ikonę i kwadrat „Zatrzymaj”; stan „Sterujesz Ty” zachowuje napis.
- Silnik gier — po niezależnym przeglądzie modułu (poważne): anulowanie zadania w trakcie wywołania silnika nie
  zostawia już połączenia w połowie wiadomości (następne wywołanie łączy się od nowa zamiast zgłaszać fałszywy
  błąd); ponowienie transakcji po przekroczeniu czasu w następnym kroku nie stosuje zmiany drugi raz.
- Silnik gier (drobne): kill-switch kończy także serwer, który dopiero się uruchamia; ramki sterujące
  WebSocket sprawdzane według RFC 6455; każdy zrzut i podgląd ma własny katalog, a w danych Alfy zostaje 50
  ostatnich.
- Przyciski kapsuły sterowania ekranem nie zmniejszają się poniżej 24 px w ciasnym pasku tytułu (cel
  kliknięcia według WCAG 2.5.8).

### Testy

- Testy kontraktowe na atrapie i na prawdziwym silniku (scenariusz: start → transakcja → odczyt → cofnięcie →
  zatrzymanie, podgląd GPU i zrzut okna); po testach katalog silnika bez zmian.
- Pliki ponad limit długości (app-api, app-core) podzielone; pełny zestaw przeglądarkowy 559/559.

## 0.0.73 — 2026-10-27

Zadania agentek przetrwają przerwę i limit (zapis postępu, kontynuacja od zapisanego miejsca), Dyrygentka zna
wybrany projekt i przekazuje wyniki między agentkami, a konto zapasowe przejmuje rozmowę od razu — z komunikatem.
Wersja zawiera też poprawki z przeglądu 27.10 (raport 39).

### Poprawki

- [krytyczne] Polecenie w `cmd` nie może już ominąć wykrywania dostępu do sieci wzorcem tekstowym (np.
  `$x -match "\\host\share"`) — wyjątek dla wzorców obejmuje tylko PowerShell, także w poleceniach zagnieżdżonych.
- [poważne] Cel zadania uruchomionego przez wyzwalacz z treścią spoza zaufania (np. z poczty) nie jest już
  traktowany jak polecenie właściciela.
- [poważne] Po wznowieniu zadania nowa pomocnica nie nadpisuje zapisu wcześniejszej (Replay i koszty zostają).
- [poważne] Zaktualizowane biblioteki `zip` (aktualizator: 9.0.5 — rozpakowanie bez bitów SUID i bez dowiązań poza
  katalog) i `tokio` 1.54.1 (ostrzeżenie RUSTSEC-2026-0386).
- Gdy konto ma limit zapytań albo odrzuca klucz, Router kończy bieżącą odpowiedź przez konto zapasowe tego samego
  dostawcy, a Alfa mówi o przełączeniu; kolejne wiadomości idą od razu przez konto zapasowe.
- Zadanie agentki po wyczerpaniu kroków albo czasu zapisuje postęp i pyta o kolejną pulę; kontynuacja rusza od
  zapisanego miejsca, także po restarcie. Czekanie na zgodę nie zużywa czasu pracy. Trzy przełączniki
  automatycznego przedłużania w Ustawienia → Agentki (domyślnie wyłączone); limity kosztów obowiązują dalej.
- Twarda odmowa kroku (Broker, Jądro) kończy zadanie z jasnym komunikatem, który krok zablokowano — agentka nie
  próbuje obejść blokady.
- Dyrygentka i agentki wiedzą, którego projektu i katalogu dotyczy rozmowa; wyłączony dostęp do katalogu nadal
  wyłącza też historię projektu.
- Kolejna agentka dostaje wyniki poprzedniej (wyciąg z faktycznych poleceń i wyników testów), a raport końcowy nie
  twierdzi, że czegoś nie uruchomiono, gdy było uruchomione.
- Pipeline'y Dyrygentki: biblioteka zapisanych pipeline'ów, edytor etapów; wynik poprzedniego etapu trafia do
  następnego także w szablonie z samym `{{input}}`.
- Profile dostępu agentek (według ról, pełny, odczyt, własny, bez narzędzi) w Ustawienia → Dostęp agentek.
- Agentki dostają mały zestaw narzędzi dobrany do zadania i wyszukiwarkę pozostałych — krótsze zapytania.
- Filtr katalogów w PowerShellu (`-notmatch '\\(obj|bin)\\'`) nie wywołuje już fałszywej prośby o dostęp do sieci.

### Testy

- Regresje: analiza powłoki `cmd`/PowerShell, cel wyzwalacza, numer pomocnicy po wznowieniu (czerwony na starym
  kodzie), przełączenie konta po `router.fallback`, szablony pipeline. Pakiet startowy interfejsu 148,9 KB gzip
  (limit 150).

## 0.0.72 — 2026-10-22

Wydanie „0.0.71B”: Alfa startuje bez okna konsoli, rozruch sygnalizuje się w programie („Rozruch OK” albo
„z problemem”) z dziennikiem do podejrzenia, a przy wolniejszym starcie nie ma już fałszywego błędu watchdoga.

### Poprawki

- Kopia uruchamiana skrótem „Alfa” nie otwiera już okna konsoli z dziennikiem — programy pomocnicze (CLI
  agentek, git, sprawdzanie wersji narzędzi) też nie migają oknami.
- Po starcie Alfa pokazuje „Rozruch OK” z czasem rozruchu, „Rozruch z uwagami” albo „Rozruch z problemem”
  (ten zostaje na ekranie) z przyciskiem „Pokaż dziennik”. Dziennik rozruchu — wpisy od startu programu
  (procesy Jądra, Broker, watchdog, moduły, głos; bez treści rozmów, sekrety zamazane) — jest w Ustawienia →
  Zdrowie systemu, z filtrem ostrzeżeń i błędów i kopiowaniem. Start, który się nie powiódł, zgłasza się przy
  następnym uruchomieniu.
- Wolniejszy start nie daje już błędu „alfa-watchdog nie zgłosił gotowości” — Alfa czeka na watchdoga równolegle
  z łączeniem z Brokerem; skrót Ctrl+Shift+F12 działa jak dotąd.

### Testy

- Regresje: watchdog gotowy po limicie (ostrzeżenie zamiast błędu, czerwony na starym kodzie), dziennik rozruchu
  (werdykt, okno, limit 500 linii), sygnał w UI (werdykty, czekanie na koniec rozruchu). Mosty CLI: przyczyną
  „nie zainstalowane” była instalacja z wnętrza zwirtualizowanej aplikacji Claude — CLI zainstalowane ponownie
  i sprawdzone w Alfie uruchomionej przez Eksplorator.

## 0.0.71 — 2026-10-22

Koszt zadań agentek liczy się do limitu właściwego konta, konto po wymianie klucza wraca do pracy, a wywołania
narzędzi od serwerów bez numeracji się nie sklejają.

### Poprawki

- [poważne] Koszt zadań agentek trafia do dostawcy (i konta), który faktycznie odpowiadał — wcześniej zapisywał się
  pod „Router”, więc limit kosztów konta go nie widział (limit miesięczny tak). Tura agentki pokazuje faktycznego
  dostawcę i model, gdy odpowiadał jeden.
- [poważne] Konto, którego klucz zniknął z Menedżera poświadczeń albo którego ostatni test się nie udał, po „Zmień
  klucz” wraca do pracy także u dostawcy, którego klucza nie da się sprawdzić z góry (własny adres zgodny z OpenAI)
  — wcześniej zostawało „nieskonfigurowane” albo w błędzie z nowym kluczem i Router je pomijał.
- Gdy klucza konta nie ma w Menedżerze poświadczeń, komunikat mówi, co zrobić („wpisz go ponownie — Zmień klucz”),
  zamiast pokazywać wewnętrzny identyfikator konta.
- Wywołania narzędzi od serwerów zgodnych z OpenAI, które nie numerują wywołań albo wysyłają pusty identyfikator,
  nie sklejają się w jedno — kolejne wywołanie zaczyna się po kompletnych argumentach poprzedniego.
- Ostrzeżenie przed limitem kosztów nazywa właściwy limit (konta, miesięczny albo zadań w tle) i nie gubi limitu
  innego zakresu, który przekroczyłoby dopiero pełne maksimum odpowiedzi.
- Cofanie kroków powłoki nie przegląda już tysięcy plików przy każdym cofnięciu (znaczniki katalogu wyników cargo
  sprawdzane tylko dla katalogów `target`); zachowanie bez zmian.

### Testy

- Regresje (sprawdzone na starym kodzie): koszt tury agentki pod faktycznym dostawcą (`app-core/tests/agent_costs.rs`
  — wcześniej `provider: "router"`), koszt i model każdej tury przebiegu, wymiana klucza konta bez klucza i w błędzie
  (rdzeń i obie atrapy), pusty `id` wywołań narzędzi. Scenariusz atrapy interfejsu `?scenario=key-missing`. Testy
  mostów CLI z dłuższym limitem odczytu wersji atrapy (losowe czerwone w pełnym biegu). Przeglądy w świeżym
  kontekście z weryfikacją każdego ustalenia — potwierdzone poprawione przed wydaniem.

## 0.0.70 — 2026-10-22

Limity kosztów kont działają, Router nie blokuje rozmowy na długo przed limitem, a Gemini i modele OpenAI
dostają poprawne wywołania narzędzi i limity odpowiedzi.

### Poprawki

- [poważne] Limit kosztów konta z kreatora (domyślnie 100 zł/mies.) jest egzekwowany — wcześniej wiersz konta go
  pokazywał, ale nic go nie sprawdzało (ani ostrzeżenia, ani blokady). Konta jednego dostawcy sumują się; dostawca
  ma limit, gdy każde jego konto go ma. Komunikat blokady mówi, że limit konta zmienia się w Ustawieniach → Konta
  i klucze API.
- [poważne] Rozmowa przez Router nie staje już przy ok. 90 % limitu kosztów: blokada liczy koszt minimalny
  wiadomości, a pełne maksimum odpowiedzi (np. 128 tys. tokenów) daje tylko ostrzeżenie. Komunikat mówi, który
  limit (miesięczny, zadań w tle, dostawcy), zamiast „Monthly”.
- [poważne] Gemini przez endpoint zgodny z OpenAI: kilka wywołań narzędzi naraz nie skleja się w jedno (agentka
  czyta kilka plików jednocześnie), a koszt modeli o nazwach `models/…` jest liczony.
- [poważne] Zadania agentek u oficjalnego OpenAI dostają 16 384 tokeny odpowiedzi (nie więcej, niż pozwala rodzina
  modelu) zamiast 8 192 — duże pliki nie ucinają się. Zwykła rozmowa zostaje przy 8 192.
- Błędy w zadaniach agentek nazywają dostawcę, który zawiódł (jak zwykła rozmowa), zamiast „Router”.
- Konto przypisane wszystkim agentkom (domyślne ustawienie kreatora) nie jest już przypięte — Router dobiera model
  do pytania (wcześniej każde krótkie pytanie szło do pierwszego, zwykle najdroższego modelu konta). Dotyczy też
  kont dodanych wcześniej; wybór części agentek przypina konto jak dotąd.
- Panel Pliki: „Zapisz jako…” tylko dla plików tekstowych (także SVG) — dla docx, xlsx i obrazów kończyło się
  błędem; zostają Otwórz, Pokaż w Eksploratorze i Kopiuj plik.
- Blok kodu w rozmowie nie ma już przycisku „Uruchom w terminalu (przez Brokera)”, który nigdy niczego nie
  uruchamiał; kod uruchamia agentka narzędziem powłoki albo Ty po skopiowaniu.
- Cofanie poleceń powłoki zapisanych przed 0.0.69 nie zgłasza fałszywego konfliktu i nie przywraca starych plików
  w `node_modules`, `__pycache__` ani w katalogu wyników cargo.

### Testy

- Regresje (większość sprawdzona na starym kodzie): wywołania narzędzi Gemini bez `index` i cena `models/…`,
  blokada według kosztu minimalnego z ostrzeżeniem według maksimum, sumowanie limitów kont, model w błędzie
  przebiegu agentki, limit wyjścia OpenAI w ciele żądania (zadanie i rozmowa), cofanie z katalogiem oznaczonym
  znacznikiem cargo; panel Plików sprawdzony w podglądzie interfejsu. Przegląd w świeżym kontekście (2 agentów)
  — poprawki w `858c90e3` i `3834ff1d`.

## 0.0.69 — 2026-10-22

Agentki pracują przez API u wszystkich obsługiwanych dostawców i na większych plikach, rozmowa sama wychodzi
z trybu offline, a kopia Alfy uruchamiana skrótem nie ma już testowego portu debugowania.

### Poprawki

- [krytyczne] Kopia Alfy uruchamiana skrótem „Alfa” nie otwiera już portu debugowania okna (CDP 9222). Wgranie
  brało plik z buildu testowego, przez który każdy program na tym komputerze — także polecenie agentki na L4 —
  mógł sterować oknem Alfy. Wgranie buduje teraz osobną kopię bez funkcji testowych z bieżącej wersji (razem
  z Jądrem) i przerywa, zanim zamknie Twoją Alfę, gdy plik ma port debugowania.
- [krytyczne] Agentka nie psuje już plików przy zapisie: odczyt ukrywa wartości podobne do sekretów (także zwykły
  kod, np. `secret: SecretName`) jako „[ZREDAGOWANO]”, a przepisanie pliku wstawiało ten znacznik zamiast kodu.
  Zapis, w którym przybywa znaczników, kończy się odmową z wyjaśnieniem dla agentki.
- [poważne] Agentki z narzędziami działają u dostawców zgodnych z OpenAI — DeepSeek, OpenRouter, Gemini, xAI,
  Mistral, Kimi, Qwen, MiniMax i Z.ai. Wcześniej Router odrzucał tam każde zadanie z narzędziami i pracowały
  tylko konta Anthropic i OpenAI. Model bez narzędzi (np. część modeli OpenRouter) przełącza na następny model.
- [poważne] Po jednym błędzie sieci Alfa nie zostaje już „offline” do restartu: „Ponów” zawsze próbuje wysłać
  wiadomości z kolejki (wcześniej odpowiadało „Nadal brak połączenia.”, a sprawdzanie kluczy w tle stało). Gdy
  sieci nadal nie ma, wiadomości dostają błąd z możliwością ponowienia.
- [poważne] Chwilowy brak sieci albo wolna odpowiedź przy ponownym sprawdzaniu działającego klucza nie wyłącza już
  konta z rozmów na dobę — wynik testu jest zapisany, konto zostaje aktywne (nowe konto i nowy klucz — jak dotąd).
- [poważne] Długa rozmowa z agentką nie przeskakuje już na model lokalny: termin pierwszej odpowiedzi rośnie
  z długością kontekstu powyżej 8 tys. tokenów (zwykła rozmowa bez zmian — przełączenie na zapas nadal w 2 s).
- [poważne] Agentka może zapisać większy plik w jednym kroku: limit odpowiedzi modelu w zadaniu wzrósł z 4 096
  do 16 384 tokenów, nie więcej niż pozwala model (u dostawców bez podanego limitu — 8 192, jak w rozmowie).
- [poważne] Zadanie agentki nie kończy się już po kilku krokach „przekroczonym budżetem tokenów”: budżet rośnie
  z „Limitem kroków zadania” (50 tys. na krok, przy 40 krokach 2 mln), a komunikat wskazuje to ustawienie.
- [poważne] Koszt zadań agentki bez przypiętego modelu jest liczony — wcześniej wychodził 0, więc limit miesięczny
  i „Limit kosztu zadania” nie widziały tych wydatków. Nazwy modeli z datą wydania (np. `gpt-5-2025-08-07`) biorą
  cenę z cennika modelu bez daty.
- [poważne] Polecenia powłoki agentki (`cargo build`, `git`, `pnpm install`) działają z katalogu projektu: kopia
  do cofania pomija katalogi zależności i wyników budowania (`node_modules`, `target` cargo, `.venv`,
  `__pycache__`), które wcześniej zawsze przekraczały limit i kończyły się odmową. Cofanie ich nie obejmuje —
  wynik polecenia mówi o tym wprost, a ich usunięcie przez agentkę jest traktowane jak trwałe.
- Komunikaty błędów API nazywają dostawcę, który zawiódł (wcześniej „Limit zapytań u dostawcy Router”), a brak
  środków na koncie ma własny komunikat zamiast „odrzucił klucz” albo „limit zapytań”.
- Po ponownym uruchomieniu Alfa mówi, że zapamiętany poziom L4 czeka na ponowne potwierdzenie (Ustawienia →
  Uprawnienia) — wcześniej wracała do L3 bez słowa.
- `cargo test` zlecony agentce idzie z ograniczoną liczbą równoległych zadań (połowa wątków procesora, najwyżej 8),
  chyba że ustawisz własną (`CARGO_BUILD_JOBS`); narzędzia programisty dostają też zmienne katalogu wyników
  i magazynów npm/pnpm.
- Opis poziomu L4 w Ustawienia → Uprawnienia mówi, o co Broker nadal pyta: operacje administracyjne, destrukcja
  zlecona głosem, ryzykowne polecenia głosowe bez rozpoznanego mówcy i wysyłka na zewnątrz po kontakcie
  z niezaufaną treścią; twarde blokady obowiązują zawsze.

### Testy

- Regresje: zapis ze znacznikiem redakcji (nadpisanie, dopisanie, nowy plik), limit wyjścia modelu (znany
  i nieznany limit), budżet tokenów, koszt tury bez przypięcia (na starym kodzie 0) i z nazwą spoza cennika,
  cennik dla nazw z datą, snapshot z `target/` i `node_modules/`, usunięcie pomijanego katalogu w skażonej sesji,
  domyślne `CARGO_BUILD_JOBS`, „Ponów” bez zewnętrznego przełączenia, stan konta po teście bez rozstrzygnięcia,
  termin dla długiego promptu i zwykłej rozmowy (na starym kodzie przełączenie na zapas), model bez narzędzi,
  narzędzia u DeepSeek i OpenRouter, nazwa dostawcy w błędzie przez Router (na starym kodzie „Router”), brak
  środków, informacja o poziomie po restarcie.
- Przeglądy w świeżym kontekście: 3 recenzentów poprawek i druga runda po poprawkach przeglądu.

## 0.0.68 — 2026-10-21

Agentki przy pracy na komputerze trzymają się wybranego stylu odpowiedzi, Dyrygentka mówi o zgodach zgodnie
z poziomem autonomii, a modele i silniki pobierają się tylko w sprawdzonej wersji.

### Poprawki

- [poważne] Procesy poboczne mowy (głos Supertonic, wykrywanie końca wypowiedzi) jawnie włączały telemetrię
  ONNX Runtime — oficjalne wydania Microsoftu mogą wtedy wysyłać zdarzenia diagnostyczne (zależnie od ustawień
  diagnostyki Windows). Teraz ją wyłączają zaraz po starcie. Oficjalna biblioteka i tak zapisuje jedno zdarzenie
  startowe, a Windows może ponownie włączyć jej zbieranie — pełne wyłączenie wymaga biblioteki bez telemetrii
  (do Twojej decyzji, dokument licencji).
- [poważne] Modele i silniki pobierają się tylko w sprawdzonej wersji: Bielik, Whisper, modele pamięci (e5,
  MiniLM), głos Gosia, rozpoznawanie mówcy i archiwa silników mają przypiętą wersję i sumę SHA-256 — wcześniej
  część pobierała się z bieżącej wersji repozytorium, a sumę zapisywano dopiero po pierwszym pobraniu (albo
  pobranie wymagało Twojej zgody). Już pobrane, identyczne pliki zostają zainstalowane; zgoda udzielona przed
  przypięciem nie zainstaluje pliku innego niż przypięty, a menedżer modeli ocenia Bielika tym samym kryterium co
  rozmowa (zgodna suma). Dla archiwum Piper, modelu rozpoznawania mówcy i pliku konfiguracji głosu Gosia wydawca
  nie publikuje sumy — przypięta suma pochodzi z pliku pobranego 21.10 (każde kolejne pobranie musi być identyczne).
  Rozmiary w menedżerze modeli poprawione według wydawców (modele pamięci 449 MiB, tokenizer MiniLM 9 MiB,
  archiwum whisper.cpp 4 MiB).
- [poważne] Rozmowa z Claude Opus 4.6 albo Sonnet 4.6 przy ustawionej swobodzie odpowiedzi (inna niż „auto”)
  mogła kończyć się błędem: API Anthropic nie przyjmuje temperatury razem z myśleniem. Temperatura idzie teraz
  tylko przy myśleniu wyłączonym.
- Styl odpowiedzi i swoboda z Ustawienia → Ogólne działają też wtedy, gdy agentka pracuje z narzędziami
  (rozmowa z Deltą, zadania w tle, umiejętności) — wcześniej tylko w zwykłej rozmowie. Sprawdzenie wyniku
  i Krytyczka zostają bez stylu i z temperaturą modelu, żeby ocena miała stałą formę; podzadanie przekazane innej
  agentce idzie bez stylu (jego wynik czyta agentka, która je zleciła).
- Dyrygentka mówi o zgodach zgodnie z poziomem autonomii rozmowy — z tej samej tabeli reguł, którą stosuje
  Broker: na L4 nie zapowiada już zatwierdzania każdej akcji, ale też nie obiecuje, że zgody nie będzie
  (Broker pyta np. o operacje administracyjne i wysyłkę po niezaufanej treści na każdym poziomie).
- Supertonic 3: obok modelu instaluje się tekst jego licencji (Open RAIL-M wymaga przekazania kopii) — przy nowej
  instalacji; wcześniejsze instalacje nie zmieniają stanu (plik towarzyszący nie decyduje o „zainstalowano”).

### Dokumenty

- Licencje 24 modeli i silników sprawdzone u wydawców (`docs/vendor/licencje-modeli-i-silnikow.md`), z listą
  spraw do Twojej decyzji: Regulamin bielik.ai (m.in. zakaz przetwarzania danych osobowych), licencja modelu
  bazowego Bielika 4.5B (Qwen Research License), DirectML (licencja Microsoftu), zakazy Open RAIL-M (Supertonic 3)
  w licencji Alfy, FFmpeg (kompilacje GPL), WeSpeaker, openWakeWord i głos Gosia (użycie komercyjne nieustalone),
  biblioteka ONNX Runtime bez telemetrii. Pochodzenie „Pocket TTS PL”: katalog nie podaje źródła wag; jedyny
  znaleziony kandydat to model społeczności (nie wydanie Kyutai).

### Testy

- Strażnik katalogu: każda pozycja do pobrania ma przypięte sumy; test manifestu Bielika (rewizja, suma);
  stan GGUF przy rekordzie innym niż przypięty, licencja jako plik towarzyszący, zgoda sprzed przypięcia
  (`tests/pinned.rs`).
- Styl i temperatura w przebiegu agentki (test nie przechodzi bez temperatury w żądaniu — sprawdzone celowo),
  notatka Dyrygentki na L3 i po obniżeniu do L1, adapter Anthropic bez temperatury przy myśleniu (nie przechodzi
  na starym adapterze — sprawdzone celowo).
- Wywłaszczenie mowy przez użytkownika: 95. percentyl < 5 ms i pojedyncza próbka < 50 ms zamiast najgorszego
  przypadku czasu ściennego (czerwony pod obciążeniem w `verify` 0.0.67); decyzja w pierwszym odpytaniu
  sprawdzana w każdej próbie.
- `verify wersja` na `77d5403c`: 16 zielonych etapów, 2 czerwone, 1 nieuruchomiony (`graf` — brak `jq`). Rust 3086 zielonych, 0 czerwonych; interfejs 478; prawdziwa aplikacja 47 + 1 pominięty, pokrycie łącza 259 z 273 (94,9 %); przeglądarka (Chromium, brak Edge) 551 + 1 — połączenie z serwerem podglądu zerwane (osobno 17/17); `cargo deny` — tylko `zip`.

## 0.0.67 — 2026-10-21

Zmianę poziomu autonomii zatwierdzoną w oknie Brokera widać od razu, a niezatwierdzona nie przejdzie już po ponownym
uruchomieniu; Alfę z oknem zgód uruchamia skrót „Alfa”.

### Poprawki

- [krytyczne] Prośba o podniesienie poziomu autonomii do L3, odrzucona w oknie Brokera albo pozostawiona bez decyzji,
  mogła zostać przyjęta jak zatwierdzona: po ponownym uruchomieniu Alfy w ciągu 15 minut nowy Broker startował
  z poziomem domyślnym L3, a strażnik autonomii brał zgodność poziomu za zgodę i trwale zastępował nim obniżenie
  właściciela (także L0). Teraz strażnik przyjmuje podniesienie tylko wtedy, gdy Broker potwierdzi zgodę na tę
  konkretną prośbę, a prośby czekające na zgodę nie przetrwają ponownego uruchomienia. Dwie karty dla tego samego
  zakresu są pamiętane osobno, a zgoda jest przyjmowana w ciągu 2 sekund (wcześniej do 15).
- [poważne] Po zatwierdzeniu L4 w oknie Brokera strona Uprawnień dalej pokazywała stary poziom, choć Broker
  przyjął L4: strona nie sprawdzała poziomu po zgodzie, a rdzeń odnotowywał zgodę tylko wtedy, gdy poziom był
  wcześniej obniżany. Teraz strona sama pokazuje nowy poziom kilka sekund po zgodzie (czeka do 10 minut — tyle żyje
  karta w oknie Brokera), a zgoda jest zapisywana i ogłaszana także na nowym profilu. Komunikat po zgodzie mówi, że
  zmianę zatwierdzono w oknie Brokera; przy prośbie dla sesji strona sprawdza tę sesję, nie aktywną.
- [poważne] Na komputerze stacjonarnym nie było skrótu do Alfy, więc uruchomiona była kopia testowa z katalogu
  budowy — bez programów Jądra obok, z Brokerem w procesie i bez okna zgód (każde podniesienie poziomu odrzucane).
  Teraz skrót „Alfa” na pulpicie i w menu Start uruchamia kopię z oknem zgód (`Alfa-uruchomiona`) i jest
  odświeżany przy każdym wgraniu; skrótu zainstalowanej Alfy (instalator) nie zmienia. Kopia testowa mówi wprost,
  że nią jest i co uruchomić.
- [poważne] Biblioteki z poprawkami bezpieczeństwa: weryfikacja certyfikatów TLS (`rustls-webpki` 0.103.16,
  RUSTSEC-2026-0382 — odrzuca łańcuchy z wycofanymi certyfikatami emSign), silnik wtyczek (`wasmtime` 36.0.19,
  RUSTSEC-2026-0379/0380) i narzędzie budowy interfejsu (`vite` 7.3.7 — tylko serwer deweloperski). Ostrzeżenia
  dla `zip` zostają (Alfa nie korzysta z podatnych funkcji; przejście na `zip` 9 to osobne zadanie).
- [poważne] Wgranie nowej wersji mogło odebrać głos: na komputerze bez pobranego ONNX Runtime budowa zostawiała
  niepełne procesy poboczne mowy (bez biblioteki i bez `smart-turn`), a wgranie zastępowało nimi komplet
  właściciela. Teraz budowa testowa bierze bibliotekę z przypiętą sumą (z „Modeli lokalnych i silników” albo
  z kopii właściciela), a wgranie odmawia przy niepełnym komplecie, zanim zamknie Alfę. Opis wersji w kopii
  testowej znów podaje commit (git odmawiał na dysku z innego komputera).

### Testy

- Rust: cały projekt 3079 zielonych (w `verify wersja` 1 czerwony test czasu decyzji pod obciążeniem — osobno zielony); clippy czysty. Strażnik autonomii: zgoda przyjęta i ogłoszona
  (także na nowym profilu), dwie karty dla jednego zakresu, prośba bez zgody nie nadpisuje obniżenia ani po
  ponownym uruchomieniu Brokera, ani aplikacji — ten test nie przechodzi na atrapie Brokera, która każdą prośbę
  uznaje za zatwierdzoną (sprawdzone celowo). Stan prośby przez IPC z prawdziwą usługą Brokera i skryptowanym
  oknem zgód: czeka, zatwierdzona, odrzucona, nieznana, zerwane łącze.
- Interfejs: vitest 478 (czekanie na zgodę, odczyt po przerwaniu), przeglądarka na Chromium 551 + 1 (aplikacja nie
  wczytała się w 30 s pod obciążeniem; osobno zielony); prawdziwa aplikacja 47 zielonych i 1 pominięty, pokrycie
  łącza 259 z 273 (94,9 %). `verify wersja`: 15 zielonych etapów, 3 czerwone (opisane wyżej testy pod obciążeniem
  i `cargo deny` — tylko `zip`), 1 nieuruchomiony (`graf` — brak `jq`).
- Skrypty: `memory-sync` 85/85 (stara pamięć drugiego komputera wstrzymana, celowy powrót do dawnej treści
  przechodzi), skrót „Alfa” zapisany na pulpicie i w menu Start. `verify` bez Edge uruchamia testy w przeglądarce na
  Chromium Playwrighta (adnotacja w raporcie) zamiast zawsze czerwonego etapu.
- Na komputerze stacjonarnym (kopia 0.0.66 w `Alfa-uruchomiona`; 0.0.67 nie była wgrana): skrót „Alfa” uruchamia tryb
  przenośny Brokera (Broker, okno zgód, watchdog); L4 zatwierdzone przez właściciela, Broker potwierdza L4.

## 0.0.66 — 2026-10-20

Poprawki po pierwszym dniu z 0.0.65: działające okno Brokera (podnoszenie poziomu autonomii), narzędzia agentek bez
ręcznego wyboru katalogu, Alfa wie, kto działa na komputerze.

### Poprawki

- [poważne] Podniesienie poziomu autonomii (np. na L4) było zawsze odrzucane: wgrana Alfa nie miała obok siebie
  programów Jądra bezpieczeństwa, więc Broker działał w procesie bez okna zatwierdzeń. Teraz Broker, okno
  zatwierdzeń i watchdog (Ctrl+Shift+F12) są wgrywane razem z Alfą — zmianę poziomu potwierdzasz w oknie Brokera.
- [krytyczne] Okno zatwierdzeń odrzucało każde fizyczne kliknięcie i klawisz jako „wejście wstrzyknięte”, więc
  nie dało się ani zezwolić, ani odmówić: źródło wejścia było sprawdzane w chwili, w której Windows go nie podaje.
  Teraz jest odczytywane z samego kliknięcia albo klawisza; odznaka ryzyka nie ucina już liter („Ryzyko: wysokie”).
- [poważne] Poziom podniesiony za zgodą w oknie Brokera (np. L4) wracał po kilku sekundach do wcześniej
  zapamiętanego obniżenia (L2). Teraz zatwierdzone podniesienie staje się nowym zapamiętanym poziomem.
- Krzyżyk i Alt+F4 w oknie zatwierdzeń zamykają je (odmowa) — tak jak przycisk „Odmów” i Esc.
- Okno zatwierdzeń Brokera w wyglądzie Alfy: karta jak w programie (obramowanie i pasek ryzyka), etykiety
  i wartości w dwóch kolumnach, przyciski i kolory z systemu wyglądu Alfy w motywie jasnym i ciemnym, zaokrąglone
  rogi i ciemny pasek tytułu; czcionki skalowane do powiększenia ekranu. Szybkie podwójne kliknięcie też działa.
- [poważne] Koszty odpowiedzi pokazywały zawsze 0,00 zł: żaden model nie miał ceny. Alfa ma teraz cennik 89 modeli 11
  dostawców (OpenAI, Anthropic, OpenRouter, Google, xAI, DeepSeek, Mistral, Kimi, Qwen, Z.ai, MiniMax), sprawdzony
  20.10 na ich oficjalnych stronach, z datą i źródłem każdej ceny — Ustawienia → Koszty i limity → Cennik modeli.
- Model bez ceny pokazuje „koszt nieznany”, a bardzo tania odpowiedź „< 0,01 zł” — zamiast mylącego „0,00 zł”.
- Nowy obowiązek „Kontrola cenników” (Delta, raz w miesiącu): sprawdza ceny na stronach dostawców i przygotowuje
  aktualizację; w Ustawieniach wklejasz ją, widzisz zmiany i zatwierdzasz „Zastosuj” — agentka sama cennika nie
  zmienia.
- Zadania w tle bez wybranego katalogu roboczego dostają domyślny katalog rozmowy (narzędzia, np. sieć dla kontroli
  cenników; „porządki” widzą pusty katalog zamiast zmyślać pliki).
- [poważne] Agentki z narzędziami (np. Delta) odpowiadały „bez narzędzi”, dopóki nie wybrałeś katalogu roboczego
  rozmowy. Teraz bez wyboru używają domyślnego katalogu rozmowy; „Bez katalogu” w oknie rozmowy nadal je wyłącza.
- Alfa jako Dyrygentka nie mówi już, że nikt nie ma dostępu do komputera — wie, która agentka obsady (Delta)
  działa na plikach, programach i systemie, i podpowiada, jak ją wywołać („Delta, …”).
- Błąd dostawcy w zadaniu agentki (limit zapytań, brak sieci, odrzucony klucz) daje ten sam komunikat co zwykła
  rozmowa — z godziną ponowienia i kolejką offline — zamiast ogólnego „zadanie przerwane”.

### Testy

- Rdzeń: rozmowa bez wybranego katalogu, Dyrygentka z informacją o obsadzie, jawne „bez katalogu”; limit zapytań
  i brak sieci w zwykłej rozmowie bez zmian.

## 0.0.65 — 2026-10-20

Seria 6: kanwa artefaktów, Arena (dwa modele obok siebie), „Naucz Alfę”, karty rozmów, edycja ustawień kont dla
firm, karty graficzne w modułach głosu — oraz naprawa regresji obowiązków agentek na modelu lokalnym z 0.0.64.

### Rozmowa

- Kanwa artefaktów: Markdown, HTML i SVG z odpowiedzi otwierasz obok rozmowy („Otwórz w kanwie”) — podgląd
  i źródło, kopia, zapis; HTML i SVG w izolowanej ramce bez skryptów i bez dostępu do programu.
- Arena: ta sama prośba do dwóch modeli naraz, odpowiedzi obok siebie z czasem, tokenami i kosztem; „Wybierz tę
  odpowiedź” ustawia wybraną gałąź (historia się nie zmienia).
- „Naucz Alfę”: wiedza z tekstu, pliku (txt, md, pdf, docx) albo tej rozmowy trafia do pamięci ze źródłem;
  treść z zewnątrz jest danymi, nigdy poleceniami.
- Karty rozmów: w panelu Sesje widok kart (obsada, ostatnia wiadomość, model, koszt) obok listy.

### Konta i karty graficzne

- Konta dla firm: organizację, projekt (OpenAI) i wdrożenia (Azure) zmienisz na istniejącym koncie.
- Moduły mowy zajmują pamięć tej karty, na której naprawdę pracują (budżet pamięci na kartę z 0.0.64).

### Poprawki

- [poważne] Obowiązki agentek na modelu lokalnym kończyły się „wykryciem pętli”: zadania z arkuszem faktów
  dostawały narzędzie przekazania zadania, choć ich polecenie tego zabrania, a koperta JSON z 0.0.64 kazała małemu
  modelowi wypełniać je w każdej turze. Takie zadania nie dostają już narzędzia przekazania; argumenty narzędzi są
  sprawdzane schematem samego narzędzia (nadmiarowe pole od modelu nie odrzuca wywołania).
- Zmiana nazwy aktywnej rozmowy nie znika już przez dwa pola edycji naraz; kontrast przycisków w stylach „next”
  i „szkło” (jasne); pasek tytułu w wąskim oknie bez nachodzących przycisków.
- Baner „Tryb deweloperski: Broker w procesie aplikacji” wyłączysz na stałe przełącznikiem w Ustawieniach →
  Uprawnienia i bezpieczeństwo (banery bezpiecznego stanu pokazują się zawsze).
- Okno startuje z mniejszym pakietem: widok kart, okienka stanu i kosztu, baner aktualizacji i błąd odpowiedzi
  wczytują się dopiero, gdy są potrzebne (start znów w budżecie 150 KB).

### Testy

- Rdzeń: przyczyna regresji obowiązków potwierdzona na prawdziwym modelu (Bielik 4.5B), testy regresji
  w zadaniach, schemacie narzędzi i wykrywaniu pętli; Arena, karty, „Naucz”, kanwa, ustawienia firm, karty mowy.
- Interfejs: nowe okna i widoki w trzech stylach i dwóch motywach; prawdziwa aplikacja — kolejne polecenia rdzenia.

## 0.0.64 — 2026-10-18

Seria 5 (10 punktów, praca na laptopie): tekst z PDF dla modelu, poprawny JSON wywołań narzędzi w modelu lokalnym,
karty licencji silników i głosów, nagłówki organizacji i projektu dla kont OpenAI, osobny budżet pamięci każdej
karty graficznej, `zip` 9, bezpieczniejsze skrypty pamięci i kopii, sprawdzenie kart na laptopie z kartą
zintegrowaną Intel i NVIDIA.

### Rozmowa i modele

- Załączniki PDF trafiają do modelu jako tekst stron po kolei (polskie znaki, kolejność stron) — z limitami
  chroniącymi przed złośliwymi plikami (rozmiar, strony, obiekty, czas w osobnym wątku); PDF z hasłem, skan
  i plik uszkodzony dają modelowi krótką notkę zamiast tekstu.
- Model lokalny (llama-server) odpowiada na wywołania narzędzi poprawnym JSON zgodnym ze schematem narzędzia;
  ustawienie pozwala to wyłączyć, gdy model działa gorzej.
- Karty licencji: przy silnikach, modelach i głosach — licencja, źródło, użycie komercyjne, wymóg wzmianki
  i uwagi (np. DirectML: dane diagnostyczne — do potwierdzenia przez Ciebie; znak wodny mowy — decyzja właściciela).

### Konta i klucze

- Konta OpenAI: opcjonalne identyfikatory organizacji i projektu (nagłówki `OpenAI-Organization`, `OpenAI-Project`);
  ten sam klucz w innym projekcie nie jest duplikatem.
- Azure OpenAI: adapter gotowy i przetestowany, ale wpis w katalogu dostawców czeka na Twoją decyzję o rejestrze
  zgodności (`providers-catalog/czeka-na-wlasciciela/`).

### Karty graficzne

- Każda karta ma osobny budżet pamięci (pula karty głównej, karty zintegrowanej i każdej osobnej) — mowa na jednej
  karcie nie blokuje modelu na drugiej; na komputerze z jedną kartą bez zmian.
- Sprawdzone na laptopie z kartą zintegrowaną Intel UHD i RTX 4050: główna karta NVIDIA, klasa LaptopCuda,
  rozpoznawanie mowy i model na CUDA, karta zintegrowana z własną pulą (pamięć współdzielona) — bez regresji.

### Bezpieczeństwo i porządek

- [poważne] `zip` 9.0.5 (RUSTSEC-2026-0367/0368/0369) w załącznikach, rozpakowywaniu silników i eksporcie `.alfa`;
  dowiązania w archiwum (także zapisane systemem DOS) zawsze odrzucane. Moduł aktualizacji przechodzi na `zip` 9
  po Twoim przeglądzie — do tego czasu `cargo deny` zgłasza `zip` 8.6.0 tylko tam.
- `scripts/memory-sync.sh` nie kasuje już plików, których nie ma u źródła (opcja `--prune`); `scripts/backup.sh`
  wymaga jawnego celu i sprawdza dysk, wolumin i wolne miejsce przed zapisem (`--dry-run`).
- Dokumenty: stan pracy na dwóch komputerach, decyzje właściciela w roadmapie i dzienniku decyzji.

### Testy

- Rdzeń: PDF (wrogie liczniki stron, strumienie, hasło, uszkodzenia), schemat JSON narzędzi (żądanie do
  llama-server), budżety pamięci na kartę (testy własności na trzech kartach), `zip` 9 (klasy porad i dowiązania
  DOS), nagłówki OpenAI i adapter Azure na sztucznym serwerze, kompletność kart licencji.
- Interfejs: karty licencji w trzech stylach i dwóch motywach, formularz kont dla firm.

## 0.0.63 — 2026-10-17

Alfa korzysta z każdej karty graficznej — NVIDIA, AMD i Intel, osobnej i zintegrowanej — a na komputerze bez
karty działa na procesorze. Na kartach AMD i Intel mowa jest rozpoznawana ponad 20 razy szybciej (na karcie od
12 GB — na karcie), a na dużej karcie także głos liczy na karcie. Sprawdzone na Radeonie RX 9070 XT (AMD); karty Intel — niesprawdzone na sprzęcie.

### Karty graficzne

- [poważne] Rozpoznawanie mowy na kartach AMD i Intel jest ponad 20 razy szybsze: pakiety „Bardzo dobry” i „Dobry”
  dostają tam Qwen3-ASR (przez `llama-server` Vulkan), bo whisper.cpp nie wydaje wersji Vulkan dla Windows. Na RX 9070 XT
  zdanie rozpoznaje się w 0,16–0,23 s; wcześniej Whisper liczył na procesorze (ok. 54 s na zdanie na laptopie).
  Na karcie AMD/Intel poniżej 12 GB Qwen3-ASR liczy na procesorze (ok. 2,4 s na zdanie), a karta zostaje dla modelu
  rozmowy — obok niego Qwen3-ASR (ok. 2,9 GB) by się pewnie nie zmieścił. Gdy model rozmowy zajmie kartę (mniej niż
  3,2 GB wolnej pamięci), Qwen3-ASR też liczy na procesorze — karta NVIDIA ma wtedy jeszcze Whisper na karcie.
- Komputer z dwiema kartami widocznymi dla Vulkana (np. laptop z grafiką zintegrowaną i osobną kartą) rozpoznaje
  mowę na głównej karcie: Qwen3-ASR dostaje `--device` głównej karty, a nie liczy na słabszej albo na obu naraz. Gdy kompilacja widzi jedną kartę
  (jedna karta albo CUDA przy NVIDIA i grafice zintegrowanej) — bez zmian i bez sprawdzania listy urządzeń.
- Głos Supertonic liczy na karcie przez DirectML, gdy karta ma własne ≥ 12 GB pamięci i pobrane są pozycje
  „ONNX Runtime 1.23.0 (DirectML)” i „DirectML 1.15.4” (Ustawienia → Modele lokalne i silniki): na RX 9070 XT
  0,58 s zamiast 0,95 s na zdanie, 538 MB pamięci karty. Gdy karta nie da rady, głos sam wraca na procesor.
  Licencja DirectML dopuszcza zbieranie danych diagnostycznych przez Microsoft — pozycja ma znaczek „Do
  potwierdzenia przez człowieka” (ADR 0028, proponowany).
- [poważne] Procesor AMD z grafiką (APU) z małą rezerwą pamięci w BIOS-ie (typowo 0,5–4 GB) nie udaje już
  osobnej karty: Alfa bierze „zintegrowana” od sterownika (DXCore), zapisuje identyfikator karty (LUID) i wersję
  sterownika. Rezerwa od 6 GB (np. Ryzen AI Max z 64 GB, Radeon 890M z 16 GB) jest niedostępna dla Windows, więc
  liczy się jak pamięć karty — taki komputer dostaje klasę i pakiety jak karta z tą pamięcią, a pamięć RAM liczy
  się bez tej rezerwy (np. 32 GB z 16 GB rezerwy to 16 GB RAM dla systemu).
- Karty AMD i Intel dostają lokalny model rozmowy od 6 GB, jak NVIDIA (wcześniej od 8 GB i tylko z procesorem
  ≥ 8 wątków); liczba wątków nie obniża już klasy — tylko opisuje kompromis. Klasa dla AMD/Intel 6–11 GB nazywa się
  „podstawowa”, dla NVIDIA — „karta NVIDIA (CUDA)”. Bez zmian: „mocna” to NVIDIA od 12 GB albo każda karta od 20 GB
  (przy 32 GB RAM), bo ciężki lokalny głos wymaga CUDA. Nowa klasa „karta zintegrowana”: mały model rozmowy na karcie
  przez Vulkan, mowa na procesorze. Mała osobna karta NVIDIA (4–5 GB) liczy przynajmniej rozpoznawanie mowy.
- Pakiety: karta zintegrowana dostaje silnik Vulkan; laptop z NVIDIA i kartą zintegrowaną — także `llama-vulkan`
  dla lekkiego modelu.
- Wolna pamięć karty, obciążenie i programy, które z niej korzystają, są mierzone licznikami Windows dla każdego
  producenta (wcześniej tylko `nvidia-smi`; temperatura i moc nadal tylko dla NVIDIA): strona Urządzenia pokazuje
  kartę AMD/Intel na żywo (na komputerze z kartą NVIDIA — tylko ją, z `nvidia-smi`), a rozpoznawanie mowy wybiera
  silnik według wolnej pamięci głównej karty, tej samej, na której liczy.
- Komunikaty głosu Supertonic (karta albo powrót na procesor) trafiają do dziennika Alfy; po instalacji albo
  usunięciu DirectML głos składa się od nowa z właściwą biblioteką.
- Opisy profili głosu A–D przy pierwszym uruchomieniu mówią, czym profile naprawdę się różnią (wcześniej B był
  „dla zintegrowanej karty”, a C „dla NVIDIA z CUDA”).

### Inne

- [poważne] Czyszczenie odpowiedzi modeli z niebezpiecznego HTML (biblioteka `ammonia`) w wersji z poprawką
  bezpieczeństwa RUSTSEC-2026-0373: funkcja `url()` w stylach CSS jest rozpoznawana bez względu na wielkość liter.
  Ostrzeżenia dla biblioteki `zip` (RUSTSEC-2026-0367/0368/0369) zostają — dotyczą funkcji rozpakowywania, z których
  Alfa nie korzysta; przejście na `zip` 9 to osobne zadanie.
- Lista wyboru (np. „Pokaż rozmowy”) otwiera się przy pierwszym kliknięciu także zaraz po wczytaniu okna — klik
  w pierwszych 0,3 s był pomijany (widać to było na szybkim komputerze, w testach w przeglądarce).
- Logowanie do narzędzia CLI, którego nie ma na komputerze, mówi wprost: „Nie znaleziono programu Claude Code
  (claude) na tym komputerze — zainstaluj go i spróbuj ponownie” (wcześniej mylące „…po podłączeniu modułu
  ui-terminal”, a po angielsku „…once its module is connected”).

### Testy

- Rust: wykrywanie (APU, Intel Arc osobna i zintegrowana, mała NVIDIA, brak karty), liczniki PDH, reguły klas,
  skład pakietów, wybór karty dla DirectML, argumenty `--dml`; clippy czysty.
- Na RX 9070 XT: odczyt DXCore i liczników (sterownik 32.0.31041.1004, 1189 MB zajęte, 22 programy na karcie);
  Supertonic procesor/DirectML — 3 przebiegi × 5 zdań; powrót na procesor przy bibliotece bez DirectML i przy
  nieistniejącej karcie; Qwen3-ASR na Vulkan — 4 zdania z Supertonic, 3 identyczne, w jednym zła forma jednego wyrazu.
- Przegląd zmian w świeżym kontekście (agenty AI: 5 obszarów, każde znalezisko sprawdzone przez 3 niezależnych
  sceptyków):
  15 potwierdzonych usterek poprawionych przed wydaniem — m.in. APU z dużą rezerwą, etykieta klasy NVIDIA,
  odświeżanie głosu po zmianie DirectML, komunikaty bez karty. Drugi przegląd poprawek: 14 kolejnych — m.in.
  rezerwa APU liczona dwa razy (jako karta i jako RAM), pomiar pamięci nie tej karty, na której liczy mowa. Trzeci
  przegląd: 4 kolejne — m.in. lista kart odczytywana co 20 ms w trakcie dyktowania, przy dwóch kartach NVIDIA
  wolna pamięć drugiej karty, ocena „mieści się na karcie” według rezerwy APU zamiast karty osobnej. Całościowy
  przegląd wszystkich zmian od 0.0.62: 8 kolejnych — m.in. Qwen3-ASR obok modelu rozmowy na karcie AMD/Intel
  6–11 GB (teraz na procesorze), 250 ms czekania na liczniki Windows przed startem dyktowania, nagłówek pakietów
  i ocena modeli przy grafice zintegrowanej, dwie takie same karty, ponowne starty głosu po usunięciu ONNX Runtime.
  Sprawdzona i odrzucona w kolejnym przeglądzie: rezerwa 3 GB dla Qwen3-ASR w budżecie modelu rozmowy (pomiar wolnej
  pamięci po starcie modelu rozmowy i tak spadał poniżej progu Qwen3-ASR). Zostaje ograniczenie: na karcie AMD/Intel
  poniżej 12 GB mowa liczy na procesorze także wtedy, gdy model rozmowy nie korzysta z karty — do osobnego budżetu
  pamięci na kartę.
- Testy w przeglądarce na Chromium Playwrighta (na komputerze stacjonarnym nie ma Edge): 467/467 — w tym nowy test
  pierwszego kliknięcia listy wyboru; wcześniej 2 czerwone (nieaktualny napis w teście strony Urządzenia i lista
  wyboru pomijająca klik w pierwszych 0,3 s).
- Prawdziwa aplikacja: 37 kolejnych poleceń łącza przećwiczonych na prawdziwym rdzeniu (Kreator agentek, skróty,
  odmowy dla nieistniejących celów przed oknem systemowym, siecią i magazynem sekretów) — pokrycie 235 z 264 (89 %,
  wcześniej 75 %); 41 testów zielonych, 1 pominięty (brak Claude Code).
- Nie sprawdzono na sprzęcie: laptop z kartą zintegrowaną, APU AMD (małe i duże rezerwy) i karty Intel (brak tych
  maszyn); DirectML na kartach NVIDIA i Intel ≥ 12 GB i na APU z rezerwą ≥ 12 GB (włącza się sam, zmierzone tylko
  na osobnej karcie AMD); na karcie zintegrowanej z małą rezerwą DirectML jest wyłączony (niezmierzony).

## 0.0.62 — 2026-10-16

Seria 3 i 4: dziewiętnaście nowości — nowe polecenia bez modelu (jednostki, daty, lata przestępne, liczby
słownie i rzymskie, procenty, wybór z listy, liczenie znaków), `F1` z instrukcją, podpowiedzi z instrukcji
w wyszukiwaniu Ustawień, szablony i polecenia jako JSON, statystyki rozmowy, wyszukiwanie na Tablicy,
kopiowanie rozmowy, zapamiętany tryb skupienia, zapis wiadomości jako szablonu i układ na wąskie okno (od
360 px) — oraz poprawki po przeglądzie kodu serii 3.

### Rozmowa

- Polecenia bez modelu: przeliczanie jednostek („Ile to 5 km w milach?”, „Zamień 30 stopni Celsjusza na
  Fahrenheita”, „100 km/h na m/s”) — długość, masa, objętość, temperatura, prędkość i czas.
- Polecenia bez modelu: rachunki na datach („Ile dni do 24 grudnia?”, „Ile dni minęło od 1 września?”, „Jaki
  dzień tygodnia był 1 stycznia 2000?”, „Jaka data będzie za 30 dni?”, „Jaki dzień będzie jutro?”).
- Statystyki rozmowy (paleta `Ctrl+K` → „Statystyki rozmowy”): wiadomości, słowa, czas czytania, tokeny, koszt,
  średni czas odpowiedzi, narzędzia, okres, inne wersje i modele — z kopią jako Markdown.
- Tablica: wyszukiwanie (bez polskich znaków, także po tytule rozmowy i agentce) i „Kopiuj wszystko (Markdown)”.
- „Zapisz jako szablon promptu” w menu Twojej wiadomości — nazwa z pierwszej linii, bez powtórzeń.
- Polecenia bez modelu: liczby słownie, także kwoty jak na fakturze („Zapisz 1999,99 zł słownie” → „… złotych
  99/100”), i liczby rzymskie w obie strony („2026 rzymskimi”, „rzymskie MCMXC”).
- Polecenia bez modelu: procenty („Ile procent to 25 z 200?”, „O ile procent wzrosło z 80 do 100?”), lata
  przestępne i długość miesięcy („Ile dni ma luty 2028?”), losowy wybór z podanej listy („Wybierz jedno:
  pizza, sushi, żurek”) i liczenie znaków i słów („Policz znaki: …”).
- Pod polem pisania dopisek „Bez modelu: …”, gdy wiadomość dostanie odpowiedź od razu, bez modelu.
- „Kopiuj rozmowę (Markdown)” w palecie — z oznaczeniem wiadomości wygenerowanych przez AI, jak w eksporcie;
  „Statystyki rozmowy” także w menu sesji na liście rozmów.

### Ustawienia i wygląd

- `F1` otwiera instrukcję; wyszukiwanie w Ustawieniach pokazuje też do trzech trafień z instrukcji.
- Szablony promptów i własne polecenia: „Kopiuj jako JSON” i „Wklej z JSON…” — bez identyfikatorów, ze
  sprawdzeniem kształtu przed zapisem (rdzeń sprawdza resztę jak przy ręcznym dodawaniu).
- Tryb skupienia (`F11`) zostaje włączony po ponownym uruchomieniu programu.
- Wąskie okno (400 px, ekrany od 360 px): pasek tytułu mieści przyciski okna, nagłówek strony kont się zawija,
  panele i okna dialogowe nie wychodzą poza okno.
- Ustawienia → Polecenia bez modelu: przykłady wbudowanych poleceń są klikalne — wpisują się do testera.

### Poprawki po przeglądzie kodu serii 3 i 4

- [poważne] Liczenie znaków działa tylko dla tekstu po dwukropku albo w cudzysłowie, a wybór z listy — tylko po
  dwukropku: zwykłe pytania („Ile liter ma alfabet polski?”, „Wybierz coś na obiad, jestem głodny”) idą do modelu
  zamiast dostać odpowiedź bez sensu.
- Liczby słownie od ok. 576 mld nie mają już błędnych miejsc po przecinku w zapisie cyfrowym; „Jaki dzień tygodnia
  będzie 29 lutego?” bez roku odpowiada dla najbliższego roku przestępnego.

- Przeliczenia z bardzo małym wynikiem („1 mg w tonach”) pokazują cyfry znaczące zamiast zera; „1 dzień”
  zamiast „1 dni”.
- „Ile dni do 29 lutego?” bez roku liczy do najbliższego roku przestępnego.
- Wyszukiwanie w instrukcji i w Ustawieniach nie pokazuje spóźnionych wyników starszego zapytania.
- Wklejanie JSON: komunikat o błędzie znika po poprawce, a „pozycja nie jest obiektem” ma własny komunikat
  (także po angielsku).
- Nazwa szablonu z wiadomości nie przecina emoji (cięcie po znakach, nie po jednostkach UTF-16).
- Test terminala nie zależy od stałego czekania 50 ms (jedyny czerwony test bramki 0.0.61 przy obciążonej
  maszynie).

### Testy

- Rdzeń: przeliczanie jednostek (w tym °F i K, prędkość, czas) i rachunki na datach (rok domyślny w przód
  i w tył, dzień tygodnia, miesiące, przesunięcia, daty niepoprawne idą do modelu); katalog poleceń.
- Interfejs: import JSON (kształt, limit, pusty i błędny JSON), statystyki, wyszukiwanie Tablicy, nazwa szablonu,
  `F1`, zapis trybu skupienia, kopia rozmowy z notą AI; E2E serii 3 i 4 w trzech stylach i dwóch motywach;
  wąskie okno 400 i 360 px (rozmowa, panele, okna, pasek tytułu, wszystkie strony Ustawień).
- Rdzeń (seria 4): liczby słownie do 999 miliardów i rzymskie (tylko postać kanoniczna), procenty i zmiana
  procentowa, lata przestępne (1900, 2000, 2028), wybór z listy w oryginalnej pisowni, liczenie znaków i słów.

## 0.0.61 — 2026-10-16

Druga seria dziesięciu nowości: instrukcja wbudowana w program, Tablica przypiętych odpowiedzi, tekst
z dokumentów Word, Excel i PowerPoint dla modelu, styl odpowiedzi, budżet zadań z wyzwalaczy, archiwum rozmów,
tłumaczenie odpowiedzi, import umiejętności SKILL.md i SBOM — oraz poprawki po przeglądzie kodu serii 1.

### Rozmowa

- Instrukcja w programie: Ustawienia → Instrukcja (albo paleta `Ctrl+K` → „Otwórz instrukcję”) — cały
  przewodnik użytkownika, działa bez internetu, z wyszukiwaniem bez polskich znaków.
- Tablica: „Przypnij do tablicy” w menu odpowiedzi agentki, a w palecie „Pokaż Tablicę” — do 40 przypiętych
  odpowiedzi z różnych rozmów z „Otwórz rozmowę”, „Kopiuj” i „Odepnij”.
- Załączniki DOCX, XLSX i PPTX trafiają do modelu jako tekst (akapity, slajdy po kolei, komórki arkuszy) — jako
  treść z zewnątrz, z limitami chroniącymi przed złośliwymi plikami.
- Styl odpowiedzi (zwięźle, szczegółowo, prostym językiem, formalnie) i swoboda (precyzyjnie, zrównoważenie,
  twórczo) w Ustawienia → Ogólne.
- „Przetłumacz na angielski” i „Przetłumacz na polski” w menu odpowiedzi agentki.
- Lista rozmów: pole „Pokaż rozmowy” (dziś, 7 dni, 30 dni) i podpowiedź z liczbą wiadomości i datą założenia.

### Agentki i zadania

- Wyzwalacze: „Tylko gdy komputer jest bezczynny” i budżet zadania (kroki, minuty, koszt) — w granicach
  polityki Marszałka; lista pokazuje budżet każdego wyzwalacza.
- Import umiejętności z pliku SKILL.md (format Agent Skills) — przez kwarantannę i zatwierdzenie z hashem,
  jak każda umiejętność z zewnątrz.

### Bezpieczeństwo i zgodność

- Plik `SECURITY.md`: jak zgłosić podatność i w jakim czasie (CRA: 24 h / 72 h / 14 dni); SBOM wydania
  w formacie CycloneDX (`node scripts/sbom.mjs`) jako etap bramki wersji. Kontakt bezpieczeństwa czeka na decyzję.

### Poprawki po przeglądzie kodu serii 1

- [poważne] Offline polecenie bez modelu nie wyprzedza już wiadomości czekających w kolejce, a po odzyskaniu
  sieci odpowiada model — wcześniejsze pytanie nie zostaje bez odpowiedzi; wiadomość z załącznikiem offline
  czeka w kolejce.
- [poważne] „Ponów” odpowiedzi w quizie nie ocenia odpowiedzi drugi raz — quiz zostaje przy tym samym pytaniu.
- [poważne] Edycja własnego polecenia albo szablonu zapisuje właściwą pozycję także wtedy, gdy lista zmieniła się
  w trakcie (zapis po identyfikatorze; inne wiersze są zablokowane podczas edycji).
- Rachunki: „100 plus 23%” = 123 jak w kalkulatorze, liczby poza dokładnością obliczeń idą do modelu, wynik
  nierzeczywisty ma właściwy komunikat.
- Własne polecenia: dłuższe wiadomości (ponad 2000 znaków) idą do modelu, odpowiedź ma limit długości, pusta
  odpowiedź nie powstaje; wyrażenia regularne liczone poza blokadą stanu.
- Edytor konfiguracji pokazuje błędy składni plików na bieżąco; identyfikatory z interfejsu są sprawdzane.

### Testy

- Rdzeń: ekstrakcja DOCX/XLSX/PPTX (polskie znaki, encje, kolejność slajdów, limity), instrukcja (każdy dział,
  wyszukiwanie), Tablica (limit, powtórki, odpinanie), styl i swoboda w żądaniu do modelu, budżet wyzwalacza,
  import SKILL.md przez walidację przepisu, regresje z przeglądu (kolejka offline, „ponów” w quizie).
- Interfejs: nowe okna i strony w trzech stylach i dwóch motywach; prawdziwa aplikacja: instrukcja i Tablica.

## 0.0.60 — 2026-10-15

Seria dziesięciu nowości: Alfa odpowiada na proste polecenia sama, bez modelu i kosztu (także bez
internetu), ma szablony promptów, własne polecenia z testerem i quiz z tabliczki mnożenia, oznacza
treści wygenerowane przez AI zgodnie z AI Act, pokazuje, gdzie trafiają dane, oraz dostała strony
Moduły i Zaawansowane.

### Rozmowa

- Polecenia bez modelu: rachunki („Ile to jest 7 razy 8?”, procenty, potęgi, pierwiastki, liczby
  słowami), literowanie słowa, kostka, moneta i liczba losowa, data i godzina — odpowiedź od razu,
  bez kosztu, także offline (wiadomość nie czeka w kolejce). Pytania podobne z wyglądu („ile kosztuje
  5 jabłek”, „która godzina w Tokio”) nadal idą do modelu.
- Quiz z tabliczki mnożenia: „Przepytaj mnie z tabliczki mnożenia przez 7” — 10 pytań, odpowiedź liczbą
  albo słowami (także głosem), „koniec” przerywa z podsumowaniem.
- Własne polecenia: nowa strona Ustawienia → Polecenia bez modelu — fraza → stała odpowiedź (cała
  wiadomość, początek z `{reszta}`, fraza w środku albo wyrażenie regularne), priorytet, tester „kto
  odpowie na tę wiadomość” i katalog poleceń wbudowanych; tam też wyłączysz polecenia bez modelu.
- Szablony promptów ze zmiennymi `{{nazwa}}`: Ustawienia → Szablony promptów oraz okno „Wstaw szablon
  promptu” z palety `Ctrl+K` — pola zmiennych, podgląd i wstawienie do pola wpisywania bez wysyłania.
- Eksport rozmowy także do JSON (obok Markdown i HTML) — z menu wiadomości, menu sesji i palety.

### Przejrzystość i prywatność

- [poważne] Oznaczanie treści AI (AI Act art. 50): wprowadzenie i stała informacja pod polem wpisywania
  mówią, że rozmawiasz ze sztuczną inteligencją; w eksporcie MD, HTML i JSON wiadomości agentek mają znak
  „AI” z modelem i znacznik maszynowy (nagłówek YAML, `meta`, typ źródła IPTC).
- Karty modeli w Ustawienia → Modele lokalne i silniki: pochodzenie, przeznaczenie, ograniczenia,
  licencja i znak „Tworzy treść AI” dla każdej pozycji katalogu.
- „Gdzie trafiają dane” (Ustawienia → Dobór modelu): kraj dostawcy, czy może uczyć modele na twoich
  danych i czy trasa obsłuży sesję prywatną — z powodem blokady; tylko do odczytu.

### Ustawienia

- Nowa strona Moduły: każdy moduł ze stanem, budżetem pamięci i procesora, zdolnościami i zależnościami;
  pięć modułów opcjonalnych (model lokalny, głos, synteza mowy, konsolidacja pamięci, przenoszenie)
  można wyłączyć od następnego uruchomienia, moduły Jądra są chronione.
- Nowa strona Zaawansowane: surowy edytor plików konfiguracji (wspólny i tego komputera) ze
  sprawdzaniem składni z numerem wiersza; zapis przechodzi pełną walidację, a klucz Jądra albo zły typ
  zostawiają poprzednią treść pliku; flaga codziennego zapisu diagnostyki.

### Testy

- Rdzeń: polecenia bez modelu bez wywołania dostawcy (także offline i z wyłączonym ustawieniem), quiz
  na prawdziwym rdzeniu, własne polecenia i szablony z walidacją, edytor konfiguracji z przywracaniem,
  wyłączanie modułów przez trzy uruchomienia, trasy zgodności na prawdziwym rejestrze, eksport JSON.
- Interfejs: każda nowa strona i okno w trzech stylach i dwóch motywach (dostępność i kontrast).

## 0.0.59 — 2026-10-15

Agentka pisze i sprawdza programy: przeglądarka Alfy obsługuje strony w pełni (przewijanie, klawisze,
listy wyboru, wstecz i odświeżanie, czekanie na wynik, konsola z błędami skryptów), a nowe narzędzia
programisty czytają repozytorium Git i uruchamiają testy projektu z czytelnym wynikiem.

### Przeglądarka Alfy

- Nowe narzędzia: przewinięcie strony albo do elementu, klawisz (Enter, Tab, Escape, strzałki…), wybór
  opcji z listy, wstecz / dalej / odśwież, czekanie na tekst i konsola strony z błędami skryptów
  (wyjątki z numerem wiersza, `console.error`, błędy ładowania) — do testowania własnych stron
  pod `http://localhost`.
- Klawisz, wybór i nawigacja przechodzą przez Brokera jak kliknięcie; konsola i czekanie tylko czytają,
  treść jest niezaufana, a sekrety w konsoli są zasłaniane.

### Narzędzia programisty

- `dev_git`: stan repozytorium, różnice, historia, gałęzie i pojedyncze zatwierdzenie — tylko odczyt.
- `dev_test`: rozpoznaje projekt (Rust, Node.js, Python, Go, .NET), uruchamia testy z limitem czasu
  i podaje liczbę zaliczonych i niezaliczonych testów z nazwami; błąd kompilacji widać od razu.

### Testy

- Przeglądarka na prawdziwym Edge z lokalnym serwerem HTTP (lista, kliknięcie, czekanie, konsola,
  historia); narzędzia programisty na prawdziwym `git` i `cargo test`.

## 0.0.58 — 2026-10-15

Alfa poznaje komputer, na którym pracuje: wie, jakie programy są zainstalowane, potrafi podłączyć
serwery MCP (pełna przeglądarka, narzędzia programisty Chrome, pliki, Git, GitHub i inne) oraz ma
dokładną diagnostykę komputera z zapisem parametrów, testami podzespołów i historią.

### Diagnostyka komputera

- Nowa strona Ustawienia → Diagnostyka komputera: zapis parametrów (procesor, pamięć z modułami, dyski
  z kondycją, woluminy, karty graficzne ze sterownikami, sieć bez adresów, bateria, temperatury) i zdarzeń
  awarii z 7 dni z oceną — niebieskie ekrany z objaśnieniem kodu, nieoczekiwane wyłączenia, błędy dysku
  przypisane do konkretnego dysku, resety USB.
- Testy podzespołów z postępem i przerwaniem: procesor (jeden i wszystkie wątki), pamięć (wzorzec
  z weryfikacją), dysk (zapis i odczyt z pominięciem pamięci podręcznej, sumy kontrolne, odczyt losowy
  4 KB; plik próbny zawsze usuwany), sieć (DNS i połączenie).
- Historia zapisów i testów z porównaniem do poprzedniego pomiaru (spadek wydajności = ostrzeżenie),
  raport Markdown do folderu `Alfa\Diagnostyka`, automatyczny zapis parametrów raz na dobę.
- Agentka może odczytać raport narzędziem `system_diagnostics` (przez Brokera) i uruchomić lekkie
  testy; testów dysków agentka nie uruchamia.

### Serwery MCP

- Nowa strona Ustawienia → Serwery MCP: gotowe przepisy (przeglądarka Playwright, Chrome DevTools,
  pliki w folderze, Git, GitHub, pobieranie stron, notatnik wiedzy) albo własny serwer; serwer startuje
  dopiero przy pierwszym użyciu.
- [poważne] Agentka używa wyłącznie narzędzi, na które się zgodzisz; zmiana opisu narzędzia po zgodzie
  blokuje je do ponownej zgody, a każde wywołanie przechodzi przez Brokera.
- Zmienne tajne (tokeny, klucze) trafiają do Menedżera poświadczeń Windows, nie do plików; serwer
  dostaje czyste środowisko bez kluczy Alfy; nieudany start pokazuje przyczynę (np. `npm ERR! 404`).

### Znajomość komputera

- Narzędzie `system_software` i opis maszyny w zasadach agentek: przeglądarki, środowiska (Node.js,
  Python, .NET, Java), kompilatory, Git, menedżery pakietów — agentka używa tego, co jest, zamiast
  zgadywać.

### Testy

- Sonda Windows sprawdzona na prawdziwym laptopie (3 dyski NVMe, w tym jeden na mostku USB), testy
  procesora, pamięci, sieci i dysku C: uruchomione na prawdziwym sprzęcie; atrapy serwera MCP i sondy
  diagnostyki jako osobne procesy; e2e stron w 3 stylach × 2 motywach.

## 0.0.57 — 2026-10-15

Terminal Alfy (także okno logowania) nie przestawia już liter przy szybkim pisaniu: przy obciążonym
komputerze wpisane „Write-Output” potrafiło dotrzeć jako „Witre-Output”.

### Terminal

- [poważne] Klawisze trafiają do terminala dokładnie w kolejności naciśnięcia — każde wysłanie czeka na
  poprzednie (wcześniej szły równolegle i pod obciążeniem mogły się wyprzedzić).

### Testy

- Test prawdziwej aplikacji z poleceniem PowerShella wpisywanym znak po znaku; testy logowania
  w przeglądarce znają 4 kroki instrukcji Claude Code.

## 0.0.56 — 2026-10-15

Zlecanie zadań przez Google Gemini (Antigravity) działa z prawdziwym narzędziem: Alfa czyta jego
odpowiedzi w formacie sprawdzonym na Twoim koncie. Wcześniejsza wersja zakładała inny układ zdarzeń
i nie odczytałaby wyniku zadania.

### Antigravity

- [poważne] Odczyt odpowiedzi `agy` zgodny z Antigravity CLI 1.2.11 (zdarzenia `init`, `step_update`,
  `result` w obiektach, kroki z numerem i stanem ACTIVE / DONE, czas w sekundach) — sprawdzony
  jednym krótkim pytaniem na koncie Google AI Pro, w pustym folderze i w piaskownicy.

### Testy

- Parser na dokładnym kształcie prawdziwego wyjścia i na zapisie z prawdziwego `agy`; fałszywe `agy`
  w testach mostu wysyła ten sam kształt.

## 0.0.55 — 2026-10-15

Logowanie do narzędzi z kontem (Claude Code, Codex, Google Gemini przez Antigravity) prowadzi teraz
krok po kroku, po polsku i z dokładnymi klawiszami, a Alfa sama rozpoznaje, że się udało. Antigravity
nazywa się na karcie „Google Gemini — Antigravity CLI” i wyjaśnia, dlaczego to ono, a nie Gemini CLI.

### Logowanie

- [poważne] Okno logowania startuje w pustym folderze Alfy, a nie w Twoim katalogu domowym. Wcześniej
  odpowiedź „Yes” wpisana w pole rozmowy Antigravity uruchamiała model, który zaczynał czytać pliki
  w profilu i prosić o zgodę na polecenia.
- Instrukcja dla każdego narzędzia osobno: co wybrać strzałkami, kiedy nacisnąć Enter, czego nie
  wpisywać. Przy Antigravity wyraźne ostrzeżenie: nie wpisuj nic w pole „>” — to rozmowa z modelem.
- Alfa rozpoznaje zalogowanie w oknie („Login successful”, „Successfully logged in”, nagłówek
  „(Google AI Pro)”), pokazuje zielony komunikat i podświetla „Gotowe”.
- Antigravity nie ma polecenia stanu, więc Alfa zapamiętuje zalogowanie wykryte w oknie — plan i dzień,
  bez adresu e-mail. Stan połączeń pokazuje „Zalogowano · plan Google AI Pro” także po ponownym
  uruchomieniu.
- Claude Code: przycisk „Wpisz /login”, gdy narzędzie od razu pokazało pole rozmowy.

### Testy

- Rdzeń: zapamiętanie zalogowania Antigravity (bez e-maila) i stan po ponownym uruchomieniu; folder
  logowania dla profili logowania. Interfejs: wykrycie zalogowania z wyjścia narzędzia, przebieg
  „Zaloguj → wykryto → Gotowe → zapamiętano” w przeglądarce.

## 0.0.54 — 2026-10-15

Krytyczka na lekkim modelu lokalnym czasem pisze werdykt z literówką („WERYFIKCJA: OK”) — Alfa brała
to za odrzucenie i niepotrzebnie zlecała poprawkę, co przy wolnym modelu wydłużało obowiązki o minuty.
Teraz werdykt z drobną literówką albo w pogrubieniu jest rozpoznawany.

### Agentki

- Werdykt Krytyczki z literówką („WERYFIKCJA: OK”) albo w pogrubieniu jest rozpoznawany — poprawna
  odpowiedź nie idzie już do zbędnej poprawki.

### Testy

- Werdykty z literówkami i w pogrubieniu Markdown; zdanie, które tylko przypomina werdykt, nadal nie
  jest werdyktem.

## 0.0.53 — 2026-10-15

Polecenia powłoki agentek, które według analizy pracują tylko w katalogu roboczym, działają teraz
z niską integralnością Windows: każdą próbę zapisu poza tym katalogiem odrzuca sam system, nawet
gdy analiza polecenia czegoś nie zauważyła. Długie odpowiedzi wyświetlają się prawie trzy razy
szybciej. Do tego testy prawdziwej aplikacji obejmują ponad 70 % poleceń.

### Bezpieczeństwo

- [poważne] Izolacja poleceń powłoki: token niskiej integralności, katalog roboczy z etykietą Low,
  pliki tymczasowe w `AppData\LocalLow\Alfa\tmp`. Polecenie czyta to co Ty (git, cargo, node
  działają), ale nie zapisze niczego poza katalogiem roboczym. Polecenia sieciowe, instalacje
  i z celami poza katalogiem działają jak dotąd — za Twoją zgodą w oknie Brokera. Wynik polecenia
  mówi, że działało w izolacji. Wyłącznik: `tools.shell.low_integrity`.
- Izolacja nie obejmuje katalogów zbyt szerokich (profil, korzeń dysku, katalogi systemu) ani
  narzędzi, które zapisują w profilu (cargo, npm, pip i podobne). Gdy izolacji nie da się
  zastosować (np. dysk exFAT), polecenie działa jak dotąd, a wynik to mówi.
- [poważne] wasmtime 36.0.18 — poprawka ostrzeżenia bezpieczeństwa RUSTSEC-2026-0363 (limity
  tabel uchwytów w modelu komponentów wtyczek).
- Zatwierdzony wpis Antigravity w rejestrze zgodności (szara trasa, domyślnie wyłączona).

### Szybkość

- Odpowiedź 100 KB wyświetla się w 7,8 ms zamiast 22–24 ms (budżet 20 ms): czyszczenie HTML idzie
  w kilku wątkach. Zabezpieczenie (ammonia) bez zmian, wynik identyczny co do znaku.

### Poprawki po przeglądzie

- Sprawdzanie logowania narzędzi: awaria narzędzia pokazuje błąd zamiast „Zaloguj się”, wartości
  z jego wyjścia są przycinane, odczyt ma limit czasu (narzędzie nie zawiesi okna).
- Okno logowania: spóźniony sygnał zamkniętej sesji nie psuje ponownej próby.
- Most Antigravity: identyfikator rozmowy sprawdzany przed wznowieniem.

### Testy

- Test prawdziwej aplikacji: dodanie i obsługa konta API (model, wyłączenie, limit, przypisania,
  kolejki, usunięcie), mosty CLI, obsada agentek, czytelne odmowy dla nieistniejących celów —
  pokrycie łącza 71,8 % (było 52,6 %); bramka pokrycia podniesiona do 70 %.
- Izolacja na prawdziwych procesach Windows: cmd i PowerShell piszą w katalogu roboczym, zapis
  obok jest odrzucany, odczyt plików systemu i katalog tymczasowy działają.
- Testy w przeglądarce odporne na przeciążoną maszynę: liczba przeglądarek dobierana do wolnej
  pamięci, ponowienie otwarcia listy wyboru, dłuższe oczekiwanie na pierwszy ekran.

## 0.0.52 — 2026-10-14

Nowe okno Ustawienia → Stan połączeń pokazuje w jednym miejscu, co jest zalogowane i działa: konta
API, narzędzia z kontem (Claude Code, Codex, Antigravity), modele lokalne, głos, Broker, internet
i mikrofon. Po zalogowaniu się do narzędzia i kliknięciu „Gotowe” Alfa sama sprawdza konto i mówi
wprost, np. „Claude Code: zalogowano (plan Max) — Alfa może zlecać zadania”. Dochodzi też konto
Google AI Pro przez Antigravity CLI — bez klucza API, świadomie włączane, z ostrzeżeniem.

### Stan połączeń

- Każda pozycja ma kolor i jedno zdanie: zielone działa, pomarańczowe wymaga Twojego ruchu (np.
  „Nie zalogowano — kliknij Zaloguj”), szare jest nieaktywne. Na górze liczniki i „Sprawdź
  wszystko”; przy każdej pozycji „Sprawdź”, „Zaloguj” albo „Szczegóły”.
- Konta API pokazują wynik ostatniego testu klucza i jego powód (klucz odrzucony, brak środków,
  limit zapytań, brak sieci). „Sprawdź” testuje klucz listą modeli — bez płatnego zapytania.
- Mosty CLI: zainstalowany? zalogowany? jaki plan? czy most jest włączony i z zatwierdzoną
  wersją? Logowanie sprawdza się samo przy otwarciu strony.
- [poważne] Po „Gotowe” w oknie logowania nic nie mówiło, czy logowanie się udało i czy Alfa może
  z niego korzystać. Teraz Alfa od razu sprawdza konto, pokazuje wynik w powiadomieniu i na karcie
  mostu („Sprawdź logowanie” jest też na każdej karcie w Mostach CLI).
- Alfa sprawdza logowanie oficjalnym poleceniem stanu narzędzia (`claude auth status`, `codex
login status`) i zostawia z odpowiedzi tylko „zalogowano / nie”, sposób logowania i plan —
  adres e-mail, identyfikator organizacji i tokeny są od razu odrzucane.

### Antigravity (konto Google AI Pro bez klucza API)

- Logowanie kontem Google we wbudowanym terminalu Alfy (Mosty CLI → Antigravity → Zaloguj się)
  — tak samo jak Claude Code i Codex.
- Most Antigravity: Alfa może zlecać mu zadania („Delta, zleć Antigravity: …”), gdy go włączysz.
  Trasa jest domyślnie wyłączona, a karta ostrzega: regulamin Antigravity (pkt 6) zabrania
  korzystania z usługi w połączeniu z produktami innymi niż Google — grozi ograniczenie albo
  zawieszenie konta. Alfa nie daje mu automatycznej zgody na polecenia (sam je odrzuca), uruchamia
  go w piaskownicy i nie dosyła wiadomości w trakcie zadania.
- [poważne] Katalogi z sesją konta Google (`.agy`, `.gemini`) dołączyły do blokad: żaden moduł,
  wtyczka ani agentka ich nie przeczyta.

### Poprawki

- [poważne] Próba obowiązków zwalniała z 62 s do kilku minut: lekki model z karty zintegrowanej
  trafiał do zwykłej pracy, a odrzucona odpowiedź wracała do tego samego modelu. Lekki model
  zostaje przy lekkich sprawach, a poprawka i druga Krytyczka idą dalej tylko wtedy, gdy jest
  mocniejszy model. Próba po poprawce: rano 58 s, wieczór 28 s.
- Raporty `verify` mają metrykę (commit, czas, sprzęt, polecenie, log), a rejestr dowodów
  `docs/DOWODY.md` powstaje z nich sam (audyt A-15).
- rustls 0.23.46 (nowe ostrzeżenia bezpieczeństwa w 0.23.45).

### Testy

- Rdzeń: `bridges_check` na atrapie narzędzi (plan Max, brak e-maila na karcie, tylko polecenia
  stanu), brak narzędzia; most Antigravity przechodzi kontrakt mostów na fałszywym CLI, osobny
  test braku kanału zgód i sterowania; parser `agy`; profil terminala `agy_login`.
- Interfejs: logika okna (9 testów, klucze tekstów w obu językach), test przeglądarkowy okna
  i przebiegu „Zaloguj → Gotowe → powiadomienie”, zrzuty w 3 stylach × 2 motywach i w wąskim oknie.

## 0.0.51 — 2026-10-14

Alfa dobiera model do trudności zadania: łatwe sprawy idą do lekkiego, taniego albo lokalnego modelu,
kod do średniego (np. lokalny Qwen 27B), a trudne do najmocniejszego (np. GPT Sol, Opus). Gdy
Krytyczka odrzuci odpowiedź, poprawkę robi mocniejszy model. Do tego poprawki wszystkich usterek
z zewnętrznego audytu z 14 października.

### Dobór modelu

- Nowa strona Ustawienia → Dobór modelu (dawne „Router i reguły — wkrótce”): włącznik, strategia
  (oszczędnie, zrównoważenie, jakość), na jakim modelu sprawdza Krytyczka, „najpierw model lokalny”,
  szczebel każdego modelu (lekki, średni, mocny — rozpoznany z nazwy, można zmienić) i „Sprawdź na
  przykładzie”: wpisujesz prośbę i widzisz, jak trudna jest i który model odpowie pierwszy.
- Ocena trudności: kod, prośba o analizę albo projekt, wiele kroków, obraz, długość, kilka pytań,
  krótka prośba w rozmowie o kodzie („popraw to”). Polskie słowa są rozpoznawane także bez ogonków.
- Kod, planowanie i sterowanie ekranem nie schodzą poniżej średniego szczebla.
- Krytyczka sprawdza odpowiedzi na lekkim modelu. Odrzucona odpowiedź wraca do poprawki na modelu
  mocniejszym niż ten, który ją napisał. Gdy lekka Krytyczka nie potrafi ocenić, druga próba idzie
  szczebel wyżej.
- Zadanie przekazane Koderce idzie kolejką „Kod”, a nie kolejką Dyrygentki.
- Kolejka ustawiona ręcznie (konta albo wspólna) ma pierwszeństwo — Alfa trzyma się jej kolejności,
  chyba że włączysz „Zmieniaj też kolejność kolejek ustawionych ręcznie”. Podniesienie po odrzuceniu
  odpowiedzi działa zawsze.
- Oś czasu rozmowy mówi, dlaczego odpowiada dany model („dobór: trudne (kod, rozumowanie) →
  szczebel mocny”).

### Bezpieczeństwo (audyt z 14.10)

- [krytyczne] STOP WSZYSTKIEGO bez potwierdzenia Brokera nie mówi już „zatrzymano”. Pokazuje trwały
  komunikat o zatrzymaniu niepełnym i podaje, czego nie potwierdzono.
- [krytyczne] Obniżony poziom autonomii (globalny i dla sesji) wraca także wtedy, gdy Broker
  uruchamia się później albo ponownie. Strażnik pilnuje go cały czas.
- [krytyczne] Po ponownym uruchomieniu Brokera watchdog dostaje jego nową tożsamość, więc
  kill-switch spoza aplikacji dalej do niego dociera.
- [poważne] Cofnięcie częściowe da się powtórzyć, także po restarcie — wracają tylko pliki, które
  poprzednio nie wróciły. Błąd zapisu dziennika cofania nie jest już przemilczany, a komunikat mówi,
  co nie wróciło.
- [poważne] Analiza poleceń powłoki widzi polecenia zagnieżdżone (`powershell -Command "…"`,
  `cmd /c`), zakodowane i `Invoke-Expression`, `git clean -f`, `robocopy /MIR`, nadpisanie przez `>`
  i `Out-File` oraz ścieżki poświadczeń zapisane wzorcem (`.s?h`).
- [poważne] Widok bezpieczeństwa nie pokazuje „izolacji” przy zerwanym łączu z usługą Brokera, opis
  okna zatwierdzeń zgadza się z jego stanem, a nieodczytany stan Brokera daje baner „stan nieznany”.
- [poważne] Rejestr silników pobocznych kończy proces tylko przy zgodnej tożsamości (pełna ścieżka
  i czas utworzenia), a nie po samym numerze i nazwie.
- Edycja oczekującej propozycji pamięci nie zatwierdza jej — trzeba kliknąć „Zatwierdź”.

### Jakość i budowa (audyt z 14.10)

- Zależności z ostrzeżeniami bezpieczeństwa podniesione: ammonia 4.2.3, Storybook 9.1.21,
  `@vitest/mocker` 4.1.11. `cargo deny` i `pnpm audit` są czyste.
- Pigułka głosowa mieści się w budżecie startowym (7,9 KB przy limicie 8 KB) — dymki bez menu edycji.
- „verify wersja” obejmuje te same bramki co CI (lint, schematy, tokeny i kontrast, typy łącza,
  budżet rozmiaru, powłoka zwykła i testowa, audyt npm). Pominięty etap daje wynik NIEKOMPLETNY,
  a nie zielony. Pokrycie łącza w prawdziwej aplikacji jest bramką.
- Plik zależności powłoki był przy starej wersji i budowa z `--locked` odmawiała — skrypt wersji
  odświeża go teraz i sprawdza.
- Nowa bramka rozmiaru: plik ≤ 400 linii, crate ≤ 8000 linii; jawne wyjątki (testy, atrapy, dane)
  mają sufit. `app-core` zszedł poniżej limitu; podzielone duże pliki interfejsu i rdzenia.
- Poprawione: test, który się nie kompilował, rozjazd schematu i uprawnień poleceń doboru, lint
  i formatowanie.

### Testy

- Rdzeń: ocena trudności (tabela polskich i angielskich próśb), szczeble i kolejność prób, router
  (8 scenariuszy, w tym pierwszeństwo kolejki ręcznej), agentka (Krytyczka na lekkim, podnoszenie
  po odrzuceniu, druga próba Krytyczki, klasa roli przy delegacji), pełna ścieżka rozmowy przez
  prawdziwy adapter, STOP WSZYSTKIEGO, strażnik autonomii, ponowienie cofania po restarcie,
  watchdog po restarcie Brokera, analiza poleceń, odcisk procesów pobocznych na prawdziwych
  procesach, edycja propozycji pamięci.
- Interfejs: logika doboru, stan Brokera (wyścig migawki, stan nieznany), strona „Dobór modelu”
  w przeglądarce i w prawdziwej aplikacji; zrzuty w 3 stylach × 2 motywach.

## 0.0.50 — 2026-10-14

„Testuj połączenie” przy koncie OpenRouter naprawdę sprawdza klucz, a kolejkę modeli można teraz ustawić
też wspólnie dla wszystkich kont naraz.

### Konta i klucze

- [poważne] „Testuj połączenie” przy OpenRouterze sprawdza klucz i mówi wprost, czy działa. Dotąd test
  kończył się wynikiem „bez testu” i wyglądało, że przycisk nic nie robi — OpenRouter pokazuje listę modeli
  każdemu, więc sama lista nie mówiła nic o kluczu. Teraz Alfa pyta OpenRoutera o sam klucz: zły klucz daje
  „klucz odrzucony”, dobry — „działa” i pod kontem limit oraz zużycie klucza
  („Konto u dostawcy: limit klucza 50,00 USD, zużyto 1,23 USD”).

### Kolejki modeli

- Nowa zakładka „Wszystkie konta” na stronie „Kolejki modeli API”: jedna wspólna kolejka dla każdej klasy
  zadań, w której można mieszać modele z różnych kont (np. najpierw model z OpenRoutera, potem z OpenAI).
  Wspólna kolejka ma pierwszeństwo przed kolejkami pojedynczych kont; gdy jest pusta, działają kolejki kont
  i wybór automatyczny — dotychczasowe ustawienia zostają bez zmian.
- Kafelki wspólnej kolejki pokazują, z którego konta jest model; kolejność zmienia się przeciągnięciem albo
  z klawiatury, jak w kolejce konta. Przycisk „Złóż z kolejek kont” układa wspólną kolejkę z kolejek
  poszczególnych kont tej klasy.
- Model do wspólnej kolejki wybiera się z listy modeli wskazanego konta (te same filtry i wyszukiwanie co
  w kolejce konta).

### Testy

- Rdzeń: sprawdzenie klucza OpenRoutera na serwerze udającym dostawcę (dobry klucz, odrzucony klucz,
  dostawca bez takiej drogi — bez ruchu sieciowego), opis limitu i zużycia; Router stawia wspólną kolejkę
  przed resztą kandydatów; pełna ścieżka rozmowy: wspólna kolejka przed kolejką konta, model, którego konto
  nie ma, jest pomijany, pusta wspólna kolejka przywraca kolejkę konta; odmowy z czytelnym komunikatem
  (nieznana klasa, nieistniejące konto, pozycja dwa razy, pusta nazwa, ponad 10 pozycji).
- Interfejs: testy zapisu i składania wspólnej kolejki, test w przeglądarce całej zakładki i testu klucza
  OpenRoutera; zrzuty w 3 stylach × 2 motywach.

### Przyczyna błędów

- Test połączenia sprawdzał klucz listą modeli. Dla dostawców, którzy oddają ją bez klucza, zwracał
  „bez testu” zamiast pytać o sam klucz.

## 0.0.49 — 2026-10-12

Okna pierwszego uruchomienia („Witaj w Alfie”) mają nowy wygląd. Z lewej strony jest lista ośmiu kroków
z postępem, z prawej — treść kroku; przyciski „Wstecz” i „Dalej” są zawsze w tym samym miejscu.

### Pierwsze uruchomienie

- [poważne] Kreator pierwszego uruchomienia przebudowany: boczna lista kroków z ikonami, oznaczeniem kroków
  opcjonalnych i paskiem postępu; do kroków już odwiedzonych wraca się kliknięciem. Okno ma stałą wysokość —
  dłuższa treść przewija się w środku, a stopka nie skacze między krokami.
- Profil głosu i poziom autonomii wybiera się kartami z nazwą i opisem; profil dobrany do sprzętu ma etykietę
  „zalecany”.
- Pomiar sprzętu pokazuje wynik w kaflach (procesor, karta, pamięć, klasa, silnik modelu) i osobno wnioski.
- Krok „Konta i klucze” ma dwie wyraźne części: modele w chmurze (klucz dostawcy) i pakiet modelu lokalnego.
- Test mikrofonu pokazuje poziom i stan („Słyszę Cię”) w jednej ramce.
- Teksty bez żargonu: zniknęły „Voice Lab” i „fala 2”; krok korpusu mówi wprost, że nagrywanie dojdzie
  w jednej z kolejnych wersji.
- W wąskim oknie lista kroków zwija się do rzędu ikon nad treścią.

### Logowanie do Claude Code i Codex

- [poważne] Krok kreatora nazywa się teraz „Konto Claude / ChatGPT” (zamiast „Mosty CLI”) i mówi prosto, po
  co jest: masz płatne konto — zaloguj się raz, a Alfa będzie mogła z niego korzystać. Każde narzędzie ma
  kartę ze stanem („zainstalowane · wersja …” albo „nie zainstalowano na tym komputerze”) i przycisk
  „Zaloguj się”.
- Okno logowania prowadzi w trzech punktach (poczekaj na adres albo przeglądarkę → zaloguj się → „Gotowe”),
  ma tytuł „Logowanie do Claude Code” i przycisk „Gotowe”. Gdy okna nie da się otworzyć albo narzędzie
  skończy z błędem, pojawia się wyjaśnienie i „Spróbuj ponownie” zamiast gołego komunikatu.
- Okno logowania podpowiada, co zrobić, gdy narzędzie (po angielsku) zapyta o zaufanie do folderu — to pierwsze
  pytanie prawdziwego Claude Code i zwykły użytkownik nie musi go rozumieć.
- Terminal w jasnym motywie stylów „Szkło” i „Next” jest jasny. Wcześniej był czarny z białym tekstem, bo
  emulator nie rozumiał kolorów tych stylów.
- Numer procesu w oknie terminala nie ma już odstępu tysięcy („Proces 29936 działa”, nie „29 936”).
- Zgłoszone „wywalenie przy logowaniu” nie było awarią programu: okno należało do instancji testowej, którą
  skrypt testów zamknął po przekroczeniu czasu (w dzienniku: otwarcie logowania 13:35:37, zamknięcie testu
  chwilę później). Instancja testowa ma teraz widoczną plakietkę „Instancja testowa — zamknie się sama,
  nie używaj tego okna”; wcześniej jedynym znakiem był tytuł okna, którego własna belka Alfy nie pokazuje.

### Głos

- [poważne] Bardzo krótkie naciśnięcie klawisza „naciśnij i mów” (wciśnięcie i puszczenie w tej samej chwili) nie
  zostawia już programu w stanie „słucham” przy zamkniętym mikrofonie. Błąd znalazł losowy test potoku głosu
  podczas sprawdzania tej wersji; przypadek jest zapisany jako stały test.

### Konta i klucze

- Karty dostawców w kreatorze „Dodaj dostawcę” opisują zasady danych zdaniami („może trenować na Twoich
  danych · prawo: Chiny”, „przechowuje dane 30 dni”) zamiast surowych znaczników katalogu
  („cn-may-train”, „unknown”). Gdy katalog nic nie mówi, karta podaje to wprost.

### Testy i instancja testowa

- [poważne] Instancja testowa uruchamiana obok Alfy właściciela zapisuje sekrety pod własnymi wpisami
  (`Alfa/test/…` w Menedżerze poświadczeń Windows) — test kont albo hasła kopii nie może już odczytać ani
  nadpisać kluczy właściciela.
- Instancja testowa dostaje własny katalog użytkownika (ustawia go powłoka w budowie testowej) i środowisko bez
  zmiennych z kluczami API. Dotąd
  katalogi robocze sesji testowych powstawały w `%USERPROFILE%\Alfa\Sesje` właściciela, a import kluczy ze
  zmiennych mógłby zrobić konto z prawdziwego klucza.
- Test prawdziwej aplikacji otwiera prawdziwy terminal (PowerShell: wpisane polecenie daje wynik), zamyka go
  i sprawdza, że program działa dalej; to samo dla okna logowania do Claude Code (bez wpisywania czegokolwiek do
  narzędzia). Każdy krok kreatora jest też zapisywany na zrzucie z prawdziwej aplikacji.
- Nowe testy w przeglądarce: lista kroków i powrót, stałe położenie stopki, karty wyboru z klawiaturą, opisy
  dostawców. Nowy skrypt zrzutów kreatora: `apps/desktop/ui/scripts/shots-onboarding.mjs` (3 style × 2 motywy,
  także wąskie okno).

### Przyczyna błędów

- Kreator pierwszego uruchomienia pochodził z pierwszych wersji i nie przeszedł przebudowy interfejsu
  z 0.0.11–0.0.45 — widać go tylko na świeżym profilu, więc nie trafiał do przeglądów ekranów.
- Magazyn poświadczeń i katalog użytkownika są wspólne dla całego konta Windows; osobny profil testu
  (`APPDATA`, `LOCALAPPDATA`) ich nie obejmował.

## 0.0.48 — 2026-10-12

Wersja naprawcza: pięć wad znalezionych testami poprzednich dwóch wersji. Najważniejsze — po zamknięciu programu
nie zostają już procesy silników, które trzymały pamięć karty graficznej.

### Modele lokalne i głos

- [poważne] Po wyjściu z programu wszystkie silniki poboczne (`llama-server`, `whisper-server`, silniki głosu,
  Smart Turn) są zamykane. Dotąd silnik modelu potrafił zostać jako osobny proces i trzymać ok. 4 GB pamięci karty;
  następne wczytanie rozpoznawania mowy trwało wtedy 54 s zamiast kilku. Jeśli program skończy się awarią, silniki
  po nim są zamykane przy następnym starcie.
- Próbka głosu i czytanie na głos przy włączonym mikrofonie nie trafiają już do rozmowy jako Twoja wypowiedź:
  na czas odtwarzania słuchanie jest wstrzymane i wraca samo po zakończeniu.

### Testy

- Naprawa procesów sprawdzona na żywo na instancji testowej: silnik modelu uruchomiony zadaniem, wyjście
  z programu, po wyjściu żadnego procesu silnika. Biblioteka rejestru ma test na prawdziwych procesach: zamknięcie
  przy wyjściu, sprzątanie po awarii, nietknięcie obcego procesu pod tym samym numerem.
- Tor akustyczny po poprawce: wypowiedź z głośnika wykryta i zakończona, głos agentki nie wraca jako wypowiedź.
- Słabe rozpoznanie mowy przez głośnik i mikrofon laptopa — zbadane nagraniem tego, co słyszy mikrofon
  (nowy test diagnostyczny): poziom sygnału dobry, ale rozpoznanie surowego nagrania też jest złe (z ośmiu słów
  zgadzają się dwa–trzy; przez tor programu jedno). Główna przyczyna leży w samym układzie głośnik–mikrofon
  laptopa, który tłumi własny głośnik, więc ta próba sprawdza teraz wykrycie i zakończenie wypowiedzi, a zgodność
  słów podaje tylko informacyjnie. Jakość rozpoznawania żywej mowy nadal wymaga próby z właścicielem.
- Pobieranie samym programem na czystym profilu: zrobione — Qwen3-ASR (2,35 GB) i silnik `llama-server` na kartę NVIDIA (0,61 GB) pobrane, sprawdzone z przypiętymi sumami i oznaczone jako zainstalowane w 13 minut
- Niestabilny test zwalniania bezczynnego modelu naprawiony (czekał za krótko na koniec żądania).
- Sprawdzanie: próba rozmowy biegnie jednym wątkiem (dwa równoległe wczytania rozpoznawania nie mieszczą się na
  karcie 6 GB — to było źródłem czerwonego etapu), oceniający raporty zna zapis daty z kropkami i imiona obsady.
- Wyjaśnione, bez zmiany w programie: rozmowa głosowa wymaga Whispera także wtedy, gdy jest Qwen3-ASR — Whisper
  jest zapasem, do którego strażnik rozpoznawania sięga przy awarii silnika głównego.

### Przyczyna błędów

- Silniki poboczne były zamykane tylko przez zniszczenie obiektu procesu, a wyjście z programu nie niszczy
  obiektów. Teraz każdy uruchomiony silnik jest zapisywany w rejestrze, który powłoka opróżnia przy wyjściu.
- Próbka głosu gra osobną ścieżką niż rozmowa, bez tłumienia echa — mikrofon słyszał ją jak rozmówcę.

## 0.0.47 — 2026-10-12

Przy wyzwalaczach widać ostatni wynik z raportem agentki, a nad polem pisania — co Alfa właśnie słyszy. Agentka
dostaje też koszty z poprzednich dni. Sprawdzanie programu objęło połowę poleceń rdzenia i prawdziwy głośnik
z mikrofonem; te biegi pokazały trzy wady, które są opisane niżej i czekają na naprawę.

### Agentki

- Przy każdym wyzwalaczu (Ustawienia → Zadania w tle i wyzwalacze) jest ostatni wynik: kiedy ruszył, jak się
  skończyło — słowem i kolorem — oraz przycisk „Pokaż raport” z tym, co napisała agentka. Wynik odświeża się sam,
  gdy zadanie się kończy.
- Dane dla zadania w tle zawierają koszty z poprzednich dni: wczoraj, ostatnie 7 dni i 7 dni wcześniej. Wzór
  „Kontrola kosztów” porównuje teraz tydzień do tygodnia.

### Głos

- Nad polem pisania pojawia się pasek „rozmowa na żywo”, gdy Alfa Cię słyszy, przetwarza wypowiedź albo mówi:
  stan słowem, mały wskaźnik głośności i tekst rozpoznawany na bieżąco. Gdy Alfa tylko czeka, paska nie ma.

### Znane wady (znalezione testami tej wersji, jeszcze nienaprawione)

- [poważne] Po zakończeniu programu silnik lokalnego modelu (`llama-server`) potrafi zostać jako osobny proces
  i trzymać ok. 4 GB pamięci karty graficznej do ponownego uruchomienia komputera albo ręcznego zamknięcia.
  Skutek zmierzony: kolejne wczytanie rozpoznawania mowy trwało 54 s zamiast kilku. Naprawa (wspólna grupa procesów
  dla wszystkich silników pobocznych) jest pierwszym zadaniem następnej wersji.
- [poważne] Mowa odtworzona z głośnika laptopa i złapana jego mikrofonem jest rozpoznawana źle (z ośmiu słów zdania
  kontrolnego zgadza się jedno). Wykrycie początku i końca wypowiedzi działa (koniec po 3 s). Nie wiadomo jeszcze,
  ile w tym głośności i akustyki laptopa, a ile toru dźwięku programu — do zbadania z nagraniem tego, co słyszy mikrofon.
- Próbka głosu z Ustawień odtwarzana przy włączonym mikrofonie trafia do rozmowy jako wypowiedź użytkownika.

### Testy

- Pełne sprawdzenie wersji 0.0.46 nowym sposobem: linter bez uwag; 2676 testów zielonych i 1 czerwony (test
  zwalniania bezczynnego modelu — niestabilny pod obciążeniem, osobno 3 z 3 zielone); przeglądarka 348 z 348;
  prawdziwa aplikacja 23 z 24 (czerwony był mój nowy scenariusz ze zbyt ostrym założeniem — poprawiony).
- Pokrycie poleceń rdzenia w prawdziwej aplikacji: 119 z 228 (52,2 %; było 38,2 %). Doszły scenariusze poleceń bez
  skutków dla danych oraz „nieznany identyfikator = czytelna odmowa z kodem” dla 21 poleceń.
- Tor akustyczny (`scripts/trial/voice-loop.mjs`, tylko w dzień): komputer odtwarza zdanie syntetycznym głosem
  i sprawdza, co program rozpoznał z mikrofonu, oraz czy głos agentki nie wraca jako wypowiedź. Wyniki — wyżej.
- Instancja testowa ma w tytule okna „instancja testowa (nie zamykaj)” i startuje z czystym stanem (stare zadania
  z poprzednich prób ruszały w tle i zajmowały kartę).
- Przeglądarka: ostatni wynik wyzwalacza i pasek rozmowy na żywo w trzech stylach i dwóch motywach.

## 0.0.46 — 2026-10-12

Wersja o sprawdzaniu: program jest teraz testowany dużo szerzej bez udziału właściciela, a pierwsze takie biegi od
razu znalazły i pozwoliły naprawić dwa błędy. Raporty z obowiązków agentek są czystsze — bez odgrywania ról.

### Agentki

- [poważne] Raport z obowiązku w tle jest samym raportem: agentka nie odgrywa już ról obsady („Dyrygentka:”,
  „Przekazuję Delcie”) i nie kończy pytaniem, na które nikt nie odpowie. Zadanie w tle dostaje razem z danymi
  wymaganą formę wyniku. Wcześniej na małym modelu lokalnym wszystkie cztery wzory miały tę wadę.

### Sieć i pobieranie

- [poważne] Kwarantanna pobrań agentek przyjmuje katalog podany w skróconej postaci Windows (np. `SPOPIE~1` — tak
  system podaje katalog tymczasowy). Dotąd taki katalog był odrzucany jako rzekome dowiązanie i pobranie się nie
  udawało. Sprawdzenie „bez dowiązań i junction” zostaje bez zmian.

### Testy

- Nowy sposób sprawdzania (plan: `docs/plan/24-samodzielne-testowanie.md`): `node scripts/verify.mjs` z poziomami
  „szybki”, „wersja” i „sprzet”; każdy bieg zapisuje raport z liczbami i z listą tego, czego nie uruchomił.
- Testy sprzętowe na tym komputerze (`scripts/verify-hw.mjs`) — dotąd oznaczone „uruchamia maszyna użytkownika”
  i nigdy nieuruchomione: prawdziwy model wykrywania mowy, koniec wypowiedzi, rozpoznawanie mowy na karcie, rozmowa
  na prawdziwych silnikach, prawdziwy mikrofon, model mówcy, embeddingi, procesy i usługi systemu, rejestr,
  znacznik pochodzenia pobranego pliku, wydajność Markdown. Pierwszy bieg: 11 etapów zielonych, 1 pominięty
  (brak wytrenowanego modelu słowa wywoławczego).
- Test prawdziwej aplikacji działa jako osobna instancja testowa obok uruchomionej Alfy właściciela (własny profil
  i port, tylko w budowie testowej) — nie trzeba już zamykać okna właściciela. Alfa właściciela działa z osobnej
  kopii programu.
- Prawdziwa aplikacja: 22 scenariusze (było 18). Doszły: przegląd 38 poleceń odczytu ze sprawdzeniem odpowiedzi
  ze schematem łącza oraz cykle życia sesji, wyzwalaczy, zadań, kosztów i ustawień. Zmierzone pokrycie poleceń
  rdzenia: 87 z 228 (38,2 %); przed tą wersją nikt go nie liczył (pierwszy pomiar: 27,2 %).
- Próby obowiązków na lokalnym modelu z automatyczną oceną (`scripts/trial/duty.mjs`): czy liczby i nazwy w raporcie
  pochodzą z danych programu i czy wynik jest raportem, a nie rozmową. Przed poprawką formy: 4 z 4 z wadą formy;
  po poprawce: 3 z 4 czyste, czwarty odrzucony błędnie przez oceniającego (poprawione).
- Pełne testy projektu dla 0.0.45 (zaległe): linter bez uwag, 946 programów testowych, 2676 testów zielonych.

### Przyczyna błędów

- Kwarantanna porównywała ścieżkę podaną ze ścieżką rzeczywistą znak po znaku; nazwa skrócona 8.3 to ten sam
  katalog pod inną nazwą. Testy pakietu omijały problem, bo same najpierw rozwijały ścieżkę.
- Wadę formy raportów przepuściłem w 0.0.45, bo oceniałem raporty wzrokiem i sprawdzałem tylko, czy dane są
  prawdziwe. Teraz ocenia to skrypt, z dwoma kryteriami.

## 0.0.45 — 2026-10-12

Historia zmian pokazuje dziesięć ostatnich wersji, a starsze czekają w zakładce „Starsze”; tę samą listę ma teraz
okno „Co nowego”. Wróciły gotowe obowiązki codzienne agentek — agentka dostaje z programu prawdziwe dane. Doszła
kula stanu rozmowy w panelu „Głos” oraz wyszukiwarka i podsumowanie miejsca na dysku w katalogu modeli.

### Aktualizacje

- Historia zmian (Ustawienia → Aktualizacje) pokazuje dziesięć ostatnich wersji do rozwinięcia. Starsze są w zakładce
  „Starsze” — dojście z zakładki nad listą i z przycisku „Starsze wersje” pod listą; filtr wagi i „Rozwiń wszystkie”
  działają w obrębie zakładki, a liczniki zakładek idą za filtrem. Zakładki obsługuje się też strzałkami.
- Okno „Co nowego” pokazuje tę samą listę co Historia zmian: bieżąca wersja rozwinięta, pozycje z wagami (kolor
  i słowo), dziesięć ostatnich wersji i zakładka „Starsze”. Wcześniej był tam sam tekst jednej wersji.

### Agentki

- [poważne] Zadanie w tle uruchomione wyzwalaczem albo harmonogramem dostaje razem z poleceniem dane z programu:
  otwarte zadania i ich stan, zadania zakończone dziś, wyzwalacze z najbliższym uruchomieniem, koszty modeli z dziś
  i z bieżącego miesiąca z limitem oraz zawartość katalogu roboczego. Agentka ma zakaz dopisywania czegokolwiek spoza
  tych danych. Zadania z sesji prywatnych nie trafiają do zadań spoza takich sesji, tytuły zadań z treści niezaufanej
  są pomijane, a zadanie oparte na treści niezaufanej danych nie dostaje wcale.
- [poważne] Drugi wyzwalacz o tej samej nazwie da się utworzyć także po ponownym uruchomieniu programu. Dotąd
  kończyło się to błędem „wyzwalacz już istnieje”, bo numeracja identyfikatorów zaczynała po starcie od zera.
- Wróciły gotowe obowiązki codzienne w formularzu wyzwalacza (ukryte w 0.0.44): przegląd dnia, podsumowanie dnia,
  porządki w katalogu roboczym i kontrola kosztów. Polecenia zostały przepisane tak, żeby opierały się na danych
  z programu; kontrola kosztów porównuje koszt z limitem miesięcznym (porównania tygodni program jeszcze nie podaje).

### Głos

- Kula stanu rozmowy w panelu „Głos”: jedno miejsce, w którym widać, czy Alfa słucha, słyszy Cię, przetwarza, czy
  mówi agentka — słowem, kolorem i pierścieniem rosnącym z głośnością. Pod stanem jest imię mówiącej agentki
  i tekst rozpoznawany na bieżąco (dotąd rdzeń go wysyłał, ale okno nigdzie nie pokazywało).

### Modele lokalne

- Katalog modeli ma wyszukiwarkę (nazwa, identyfikator, licencja; bez polskich znaków i wielkości liter), wybór
  kolejności (jak w katalogu, po nazwie, od największych, od najmniejszych) oraz podsumowanie: ile miejsca zajmują
  pozycje na dysku i ile zajęłyby te, które można jeszcze pobrać.

### Testy

- Próba obowiązków na lokalnym modelu (Bielik 4.5B, profil tymczasowy z kopią modelu, dwa wyzwalacze jako stan
  programu): „Poranny przegląd dnia” — raport w 85 s, wyłącznie z danych (brak zadań, trzy istniejące wyzwalacze
  z prawdziwymi terminami, koszty 0,00 zł); „Podsumowanie dnia” — raport w 3 min 8 s, zgodny z danymi, z jednym
  dopowiedzeniem („jutro” przy wyzwalaczu ręcznym) — arkusz mówi teraz wprost „bez harmonogramu”; „Kontrola
  kosztów” — liczby zgodne z danymi (0,00 zł, limit 100 zł, 0 %), ale model zamiast krótkiego raportu opisywał
  kolejne kroki, więc polecenie żąda teraz trzech krótkich punktów.
- Arkusz faktów: 5 testów (komu przysługuje, pusty program, pełny arkusz, obcinanie długich list, prywatność)
  i test rdzenia — zadanie z wyzwalacza dostaje arkusz.
- Przeglądarka: historia z 17 wersjami (dziesięć + zakładka, klawiatura, filtr), okno „Co nowego” z listą, kula
  stanu przez wszystkie stany, katalog modeli (szukanie, kolejność, miejsce) — każdy scenariusz w trzech stylach
  i dwóch motywach z kontrolą dostępności. Cały zestaw: 342 z 342; testy interfejsu 336 z 336.

### Przyczyna błędu

- Identyfikator wyzwalacza powstawał z nazwy i licznika trzymanego tylko w pamięci programu; po restarcie licznik
  wracał do zera i trafiał na zapisany wyzwalacz. Wyszło w próbie obowiązków na profilu z wcześniejszymi wyzwalaczami.

## 0.0.44 — 2026-10-12

Ostatnia z jedenastu wersji z nocy 11/12 października. Gotowe obowiązki codzienne agentek zostały ukryte, bo próba
na lokalnym modelu dała raport z wymyślonymi danymi. Wszystko, co doszło tej nocy (wersje 0.0.34–0.0.44: rozmowa
głosowa na lokalnych silnikach, „Sprawdź silniki”, karty graficzne na żywo, „Wstecz / Dalej”), jest w Historii zmian.

### Agentki

- [poważne] Gotowe obowiązki codzienne (dodane w 0.0.40) nie są już pokazywane w formularzu wyzwalacza. Próba
  wykonania „Porannego przeglądu dnia” na lokalnym modelu Bielik 4.5B pokazała, że mechanizm działa (wyzwalacz →
  zadanie → agentka → zakończone po 4 min 19 s), ale agentka nie ma dostępu do prawdziwych zadań i kosztów i na
  pustym profilu napisała raport z nieistniejącymi zadaniami i kwotami. Wzory wrócą, gdy program będzie podawał
  agentce fakty razem z poleceniem. Własne wyzwalacze działają jak dotąd.

### Testy

- Pobieranie samym programem na świeżym profilu: Smart Turn i ONNX Runtime (23 s) oraz Supertonic 3 (16 plików,
  383 MB, 101 s) — wszystkie z przypiętymi sumami; po pobraniu koniec wypowiedzi przeszedł na Smart Turn, a głos
  przeszedł próbę.
- Wykonanie obowiązku na lokalnym modelu (profil tymczasowy z kopią modelu) — wynik opisany wyżej; druga próba,
  z zakazem zmyślania w poleceniu, nie skończyła się w 7 minut.
- Formularz wyzwalacza: wzory ukryte, własny wyzwalacz tworzy się jak dotąd (przeglądarka).

### Przyczyna błędu

- Wzory obowiązków dodałem w 0.0.40 bez uruchomienia ich na modelu — sprawdzone było tylko tworzenie i harmonogram.
  Polecenia prosiły o dane (zadania, koszty), których agentka w zadaniu w tle nie dostaje.

## 0.0.43 — 2026-10-12

Stan silników rozmowy pod ręką w panelu „Głos”.

### Głos

- Boczny panel „Głos” (Alt+6) pokazuje zwartą kartę „Silniki rozmowy”: który silnik rozpoznaje mowę, co wyczuwa
  koniec wypowiedzi, który głos mówi, stan bezpiecznika z przyciskiem „Zamknij bezpiecznik” i przycisk „Sprawdź
  silniki”. Pełna karta z telemetrią zostaje w Ustawieniach → Głos.

### Testy

- Test płynności strumienia odpowiedzi był czerwony od dawna, bo liczył także wstawianie całych tur do listy.
  Teraz mierzy to, co ma mierzyć: tekst odpowiedzi jest aktualizowany najwyżej raz na klatkę obrazu (sześć
  przebiegów z rzędu zielonych).
- Stan końcowy nocy: pełne testy projektu 945 programów testowych bez błędu (wersja 0.0.42), testy interfejsu
  328 z 328, testy w przeglądarce 324 z 324, test prawdziwej aplikacji 18 z 18.

## 0.0.42 — 2026-10-12

Język rozpoznawania mowy do wyboru i wyszukiwarka w ściągawce skrótów.

### Głos

- [poważne] Nowe ustawienie „Język rozpoznawania mowy” (Ustawienia → Głos): polski (domyślnie), angielski albo
  automatycznie. Dotąd silnik zawsze zgadywał język, co przy krótkich zdaniach bywa zawodne; wskazany język trafia
  też do strażnika, który sprawdza wynik w nieoczekiwanym języku drugim silnikiem. Działa od następnego włączenia
  mikrofonu.

### Interfejs

- Ściągawka skrótów (Ctrl+/) ma wyszukiwarkę: po nazwie polecenia („pamięć”, „wstecz”) albo po klawiszach
  („alt+1”, „ctrl k”), bez rozróżniania wielkości liter i polskich znaków. Gdy nic nie pasuje, okno mówi to
  wprost.

### Testy

- Pełne testy całego projektu po wersji 0.0.41: 945 programów testowych, zero błędów.
- Nowe: język w ustawieniach rozmowy (zakres, złe typy), wyszukiwanie skrótów (jednostkowo i w przeglądarce
  w obu motywach z kontrolą dostępności); zrzuty w trzech stylach.

### Przyczyna błędu

- W trakcie nocy zabrakło miejsca na dysku: każda nowa wersja przebudowuje cały projekt, a stare pliki pośrednie
  zostają (121 GB po ośmiu wersjach). Usunąłem tylko odtwarzalne pliki budowy testów; dane i program są nietknięte.

## 0.0.41 — 2026-10-12

Jeden przycisk sprawdza, czy rozmowa głosowa zadziała — bez mikrofonu i bez dźwięku.

### Głos

- [poważne] Na karcie „Silniki rozmowy” jest przycisk „Sprawdź silniki”. Naprawdę uruchamia każdy silnik:
  wczytuje model rozpoznawania mowy, każe modelowi Smart Turn ocenić sekundę szumu i robi próbną syntezę głosu.
  Wynik — „działa” albo „nie działa” z czasem i przyczyną — zostaje na karcie. Nic nie jest nagrywane ani
  odtwarzane. Na tym laptopie: rozpoznawanie 2,5 s (wczytanie modelu na kartę), koniec wypowiedzi 0,2 s,
  głos 1,9 s.
- Próba nie zabiera karty trwającej rozmowie: przy zajętej pamięci karty model próbny wczytuje się na procesor.

### Testy

- Próba na prawdziwych silnikach (`live_talk.rs`, test `self_test_runs_every_engine`): wszystkie trzy silniki
  odpowiedziały. W przeglądarce: trzy wyniki z czasem, scenariusz z brakującym Smart Turn pokazuje „nie działa”
  z przyczyną; zrzuty w trzech stylach i dwóch motywach.

## 0.0.40 — 2026-10-12

Pierwszy krok obowiązków codziennych agentek: gotowe obowiązki do wyboru jednym kliknięciem.

### Agentki

- W Ustawienia → „Zadania w tle i wyzwalacze” są cztery gotowe obowiązki codzienne: „Poranny przegląd dnia”
  (Alfa, dni robocze o 8:00), „Podsumowanie dnia” (Beta, 18:00), „Porządki w katalogu roboczym” (Gama, soboty)
  i „Kontrola kosztów” (Delta, poniedziałki). Wybranie obowiązku wypełnia formularz — porę, agentkę i polecenie
  można zmienić przed utworzeniem; podgląd pokazuje najbliższe uruchomienia.
- Obowiązek „porządki” ma w poleceniu wprost: niczego nie usuwać ani nie przenosić bez zgody.
- Poczta i kalendarz jako obowiązki wymagają łączników — zostają w planie (P14), nie ma ich jeszcze na liście.

### Testy

- Szablony: harmonogramy, agentki, teksty w obu językach, zakaz usuwania bez zgody; w przeglądarce — oba motywy
  z kontrolą dostępności, wybór z klawiatury, utworzenie i obecność na liście; zrzuty w trzech stylach.
- Samego wykonania obowiązku przez model tej nocy nie uruchamiałem — sprawdzone jest tworzenie i harmonogram.

## 0.0.39 — 2026-10-12

Przy modelach rozmowy widać, czy zmieszczą się na karcie graficznej; rozmowa sprawdzona na prawdziwym mikrofonie.

### Modele i silniki

- W „Modele lokalne i silniki” każdy model rozmowy ma ocenę: „mieści się na karcie”, „częściowo na karcie —
  wolniej” albo „liczy procesor”. Reguła: rozmiar pliku i 1,5 GB zapasu muszą zmieścić się w pamięci karty
  głównej.

### Testy

- Próba na prawdziwym mikrofonie laptopa, po cichu (`app-core/tests/live_mic.rs`, `ALFA_LIVE_MIC=1`): rozmowa
  startuje na urządzeniu dźwiękowym z prawdziwymi silnikami, słucha pięć sekund i się wyłącza. Potok zaczął
  słuchać, wykrył dźwięk otoczenia, Smart Turn ocenił koniec (98 %, 44 ms), a strażnik nie przepuścił szumu do
  czatu. Nic nie było odtwarzane ani zapisywane. Nadal niesprawdzone: żywa mowa człowieka do mikrofonu i tłumienie
  echa, gdy agentka mówi z głośnika.
- Ocena dopasowania do karty: jednostkowo (granice, brak karty) i w przeglądarce na obu modelach Bielik.

## 0.0.38 — 2026-10-12

Rozpoznawanie mowy dobiera silnik do zajętości karty graficznej — według pomiarów z tego laptopa.

### Głos

- [poważne] Gdy model rozmowy zajmie kartę graficzną, Alfa wybiera silnik rozpoznawania, który da radę: Qwen3-ASR
  na karcie przy co najmniej 3,2 GB wolnej pamięci; sam Whisper na karcie przy co najmniej 1,6 GB; w pozostałych
  przypadkach Qwen3-ASR na procesorze. Powód: zmierzone czasy dla zdania 4-sekundowego — Qwen3-ASR na karcie
  0,26 s, na procesorze 2,4 s, a Whisper turbo na procesorze 54 s.
- [poważne] Zapas Whispera po nieudanym starcie na karcie używa małego modelu (jeśli jest pobrany), a nie dużego,
  który na procesorze jest za wolny do rozmowy.

### Modele i silniki

- Pakiety Wzorcowy, Bardzo dobry i Dobry zawierają mały model Whisper (181 MB) jako zapas na procesor.

### Testy

- Plan wyboru silnika dla sześciu przypadków zajętości karty; model na procesor w argumentach silnika; skład
  pakietów. Pomiar na prawdziwych silnikach: `qwen_live` z `ALFA_QWEN_CPU=1` i program `whisper-cli`.

## 0.0.37 — 2026-10-12

„Wstecz” i „Dalej” jak w przeglądarce oraz liczniki modelu końca wypowiedzi.

### Interfejs

- Alt+← wraca do poprzedniego miejsca, Alt+→ idzie dalej: między stronami Ustawień, rozmową i sesjami. Działają
  też boczne przyciski myszy. Program pamięta 50 ostatnich miejsc; nowe miejsce po cofnięciu ucina gałąź „Dalej”.
  Oba skróty są w tabeli skrótów (można je zmienić) i w palecie poleceń.

### Głos

- Karta „Silniki rozmowy” pokazuje pracę modelu Smart Turn: ile razy oceniał koniec wypowiedzi, ostatnią pewność
  i czas oceny. Widać, czy model naprawdę pracuje, czy odpowiada zapasowa ocena z tekstu.

### Testy

- Historia miejsc: kolejność, ucinanie gałęzi, limit, usunięte sesje; w przeglądarce — klawiatura, boczne przyciski
  myszy, powrót do poprzedniej sesji, obecność w tabeli skrótów i palecie.
- Rozmowa „na sucho” potwierdza liczniki: w dwóch wypowiedziach model Smart Turn oceniał cztery razy, bez błędu,
  ostatnia ocena 93 % w 22 ms.
- Test prawdziwej aplikacji (wersja 0.0.36): 18 z 18, w tym nowe karty na prawdziwym rdzeniu.

## 0.0.36 — 2026-10-12

Widać, co dzieje się na karcie graficznej — pierwszy krok panelu sprzętu z planu P2.

### Modele i silniki

- Ustawienia → Urządzenia pokazują kartę „Karty graficzne na żywo”: ile pamięci karty jest zajęte i wolne,
  obciążenie, temperaturę, pobór mocy oraz programy, które z karty korzystają — z oznaczeniem silników Alfy
  i sumą pamięci, którą zajmują. Odczyt odświeża się co 3 sekundy; można go wstrzymać.
- Odczyt działa dziś dla kart NVIDIA (narzędzie sterownika). Na innym sprzęcie karta mówi to wprost, zamiast
  pokazywać puste pola; karty AMD i Intel dojdą przez liczniki systemu Windows.

### Testy

- Odczyt kart: nazwy z przecinkiem, brakujące pomiary, powtórzone procesy, kolejność (Rust); w przeglądarce — oba
  motywy z kontrolą dostępności, wstrzymanie z klawiatury, scenariusz bez pomiaru; zrzuty w trzech stylach.
- Test prawdziwej aplikacji: nowe sprawdzenie kart „Silniki rozmowy” i „Karty graficzne na żywo” na rdzeniu
  (odczyt z prawdziwego sterownika).
- 229 poleceń łącza (doszło `device_gpu_live`).

## 0.0.35 — 2026-10-12

Rozmowę widać i da się ją stroić: karta „Silniki rozmowy”, ustawienia końca wypowiedzi i nowe silniki w pakietach.

### Głos

- [poważne] Ustawienia → Głos pokazują kartę „Silniki rozmowy”: który silnik rozpoznaje mowę (i czy liczy na
  karcie graficznej), co wyczuwa koniec wypowiedzi, które głosy są gotowe. Gdy czegoś brakuje, karta mówi, co
  pobrać, i prowadzi do „Modele lokalne i silniki”.
- [poważne] Strażnik rozpoznawania ma podgląd: stan bezpiecznika, liczba wypowiedzi, typowy czas rozpoznania,
  najwolniejsze 5 %, ile razy rozpoznał silnik zapasowy. Po zadziałaniu bezpiecznika przycisk „Zamknij bezpiecznik”
  od razu przywraca silnik główny.
- Cztery nowe ustawienia rozmowy: wyczucie końca wypowiedzi (Smart Turn) włącz / wyłącz, pewność końca wypowiedzi
  (30–90 %), najdłuższe czekanie w ciszy (0,5–6 s) i wybór silnika rozpoznawania („najlepszy dostępny” albo
  „tylko Whisper”). Działają od następnego włączenia mikrofonu.
- [poważne] Qwen3-ASR sam wybiera kartę albo procesor: przed startem mierzy wolną pamięć karty i gdy jest jej
  mniej niż 3,2 GB (zajął ją model rozmowy), liczy na procesorze zamiast wypychać inne modele z karty.
- [poważne] Silniki rozgrzewają się przy włączeniu mikrofonu, a nie przy pierwszym zdaniu: model rozpoznawania
  wczytuje się w tle, a proces głosu startuje i robi próbną syntezę. Wcześniej pierwsza wypowiedź czekała na
  wczytanie modelu z dysku (zmierzone: do 20 sekund po zimnym starcie komputera). Teraz na tym laptopie od końca
  mowy do przekazania tekstu mija ok. 0,2 s, a odpowiedź słychać po ok. 1 s (pierwszą po 1,6 s) — bez czasu
  myślenia modelu rozmowy.
- Silnik Qwen3-ASR po 10 minutach bez użycia sam się zamyka i zwalnia pamięć karty (sprawdzane co minutę).
- Imiona agentek zapisane przez rozpoznawanie mowy po angielsku („Alpha”, „Gamma”, „Betta”) trafiają do
  właściwej agentki.

### Modele i silniki

- Pakiety jednym kliknięciem zawierają nowe silniki głosu: Wzorcowy — Qwen3-ASR, Supertonic 3, Smart Turn
  i bibliotekę ONNX Runtime; Bardzo dobry i Dobry — Supertonic 3, Smart Turn i bibliotekę; Zrównoważony — Smart
  Turn i bibliotekę.

### Interfejs

- Poprawiona nieaktualna uwaga przy głosach agentek (mówiła, że lepszy głos to „osobny etap”).

### Testy

- Pełne testy całego projektu (698 programów testowych) przeszły po wersji 0.0.34.
- Nowe: widok silników i strażnika (Rust), ustawienia rozmowy z zakresami, wybór karta / procesor, pomiar wolnej
  pamięci karty, pisownia imion; w przeglądarce — karta w obu motywach z zamknięciem bezpiecznika z klawiatury
  i scenariusz z brakującymi silnikami; zrzuty w trzech stylach i dwóch motywach.
- Rozmowa „na sucho” na prawdziwych silnikach (`app-voice/tests/live_talk.rs`, uruchamiana z `ALFA_LIVE_TALK=1`):
  nagranie zamiast mikrofonu przechodzi przez wykrywanie mowy, Smart Turn i Qwen3-ASR do czatu, a odpowiedź jest
  wypowiadana głosem Supertonic. Dwie wypowiedzi z rzędu rozpoznane bezbłędnie. Rozmowy z żywym mikrofonem nadal
  nie sprawdzałem — to wymaga człowieka przy komputerze.
- Testy w przeglądarce: 313 z 314 zielonych (czerwony ten sam co dotąd — pomiar płynności strumienia).
- 228 poleceń łącza (doszło `voice_engines`).

## 0.0.34 — 2026-10-11

Alfa słyszy, czy skończyłeś mówić — trzeci duży krok przenoszenia rozmowy z Delty AI.

### Głos

- [poważne] Koniec wypowiedzi ocenia model Smart Turn v3.2, który słucha brzmienia ostatnich 8 sekund, zamiast
  samego liczenia ciszy. Gdy model jest pewny końca, Alfa odpowiada po ćwierć sekundy ciszy; gdy słyszy, że zdanie
  jest urwane, czeka do 2 sekund i co 350 ms sprawdza ponownie. Na tym laptopie ocena trwa 25–45 ms i wynik zgadza
  się z wzorcem wydawcy. Bez pobranego modelu wszystko działa jak dotąd (ocena z tekstu).
- Model Smart Turn (ok. 9 MB) i biblioteka ONNX Runtime (78 MB) są w Ustawieniach → „Modele lokalne i silniki”,
  z przypiętymi sumami kontrolnymi. Głos Supertonic znajduje bibliotekę także po pobraniu z katalogu — nie musi
  już leżeć obok programu.
- [poważne] Naprawione dwa testy zmiany głosu agentki, czerwone od wersji 0.0.30 (nie były wtedy uruchomione):
  atrapa głosu rozróżnia teraz agentki także przy barwach Supertonic.

### Testy

- `lib-smart-turn` (nowy): protokół na udawanym modelu, cechy dźwięku — 7 testów.
- `voice-turn-impl`: klient procesu, powrót do oceny z tekstu po błędzie, przerwa po trzech błędach, ponowna ocena
  w ciszy, czasy rozmowy 250 / 350 / 2000 ms; próba na prawdziwym modelu ze wzorcem (`smart_turn_live`).
- `voice-pipeline-impl`: model dostaje nagranie 16 kHz nie dłuższe niż 8 s.
- Aplikacja nie była budowana (budowa na koniec nocy).

### Przyczyna błędu

- Biblioteka `tract` liczy skwantowany model Smart Turn inaczej niż ONNX Runtime (0,33 zamiast 0,95 dla tego
  samego nagrania), dlatego model działa w osobnym procesie z ONNX Runtime — jak głos Supertonic.

## 0.0.33 — 2026-10-11

Rozpoznawanie mowy Qwen3-ASR ze strażnikiem jakości — drugi duży krok przenoszenia rozmowy z Delty AI.

### Głos

- [poważne] Rozmowa głosowa i dyktowanie rozpoznają mowę modelem Qwen3-ASR, gdy jest pobrany (Ustawienia →
  „Modele lokalne i silniki”, ok. 2,5 GB). Na tym laptopie zdanie jest rozpoznane po ok. 0,3 s; pierwszy start
  silnika trwa ok. 2,5 s. Bez modelu wszystko działa jak dotąd na Whisperze.
- [poważne] Strażnik rozpoznawania: nagrania, w których nic nie ma (cisza, brak sygnału, krócej niż ćwierć sekundy),
  nie trafiają do modelu; wynik w nieoczekiwanym języku, zapętlony albo pusty mimo wyraźnej mowy jest sprawdzany
  drugim silnikiem; znane zmyślenia na ciszy („dziękuję za oglądanie”) są odrzucane.
- [poważne] Bezpiecznik: po trzech awariach z rzędu silnik główny jest odstawiany na 30 sekund, a rozmowę
  przejmuje Whisper — bez utraty wypowiedzi, bo nagranie jest trzymane do końca.
- Silnik zwalnia pamięć karty po 10 minutach bez użycia (mechanizm jest; wywołanie z zegara dojdzie z ekranem
  „Rozmowa na żywo”).

### Czego nie sprawdzono

- Rozmowy głosowej na żywo z mikrofonem. Sprawdzone jest rozpoznawanie nagrań prawdziwym modelem (test na żywo)
  i całe przełączanie silników na udawanych silnikach.
- Aplikacja w tej wersji nie była budowana ani uruchamiana (budowa co pięć dużych ulepszeń albo na polecenie).
- Wybór karta / procesor nie patrzy jeszcze na wolną pamięć karty: Qwen3-ASR startuje na karcie zawsze, gdy
  jest silnik na kartę. Razem z dużym modelem rozmowy na 6 GB może się nie zmieścić — wtedy zadziała bezpiecznik.
- Stanu bezpiecznika i telemetrii nie widać jeszcze w oknie.

### Testy

- 12 testów strażnika i bezpiecznika, 4 testy części silnika bez sieci, 7 testów opakowania z silnikiem zapasowym,
  test na prawdziwym modelu (pomijany bez plików), test katalogu. vitest 311/311.

### Przyczyna błędu

- Reguła odrzutu ciszy przeniesiona z Delty liczyła tło z najcichszych ramek, więc mowa ciągła bez żadnej pauzy
  wychodziła jako „0 ms sygnału”. W Alfie przy wyraźnie słyszalnym tle obowiązuje próg stały.

## 0.0.32 — 2026-10-11

Nowszy silnik modeli lokalnych w katalogu pobrań — potrzebny do rozpoznawania mowy Qwen3-ASR.

### Modele lokalne

- [poważne] Katalog pobiera `llama-server` w wydaniu b11559 (było b6710) dla procesora, Vulkan i kart NVIDIA.
  Stare wydanie nie umiało wczytać modelu rozpoznawania mowy Qwen3-ASR („unknown model architecture”).
- [poważne] Archiwa silnika mają przypięte rozmiary i sumy SHA-256 z listy plików wydania — pobranie jest
  sprawdzane bez pytania o zgodę na policzony hash. Dla wersji na karty NVIDIA licencja bibliotek pozostaje
  do potwierdzenia przed wydaniem publicznym.

### Pomiary (bez zmian w programie)

- Rozpoznawanie mowy Qwen3-ASR na tym laptopie: 0,43 s na zdanie (mediana), wszystkie 40 nagrań rozpoznane jako
  polskie; poza dwoma łamańcami językowymi 3 błędne słowa na 120 przy głosie Supertonic i 5 przy Piperze.

### Czego nie sprawdzono

- Nowy silnik sprawdziłam tylko z modelem rozpoznawania mowy (wersja CUDA, pobrana i rozpakowana ręcznie). Rozmowy
  z modelem tekstowym na nowym silniku ani pobrania przyciskiem w Ustawieniach nie sprawdzałam.
- Aplikacja w tej wersji nie była budowana ani uruchamiana (budowa co pięć dużych ulepszeń albo na polecenie).

### Testy

- Testy katalogu (12 grup) zielone; test instalacji z archiwów zdejmuje przypięte sumy, bo używa archiwów
  zbudowanych na miejscu.

## 0.0.31 — 2026-10-11

Każdej agentce wybierasz głos: dziesięć barw Supertonic 3 w Ustawieniach → Głos.

### Głos

- Cztery nowe ustawienia „Głos Alfy”, „Głos Bety”, „Głos Gamy”, „Głos Delty”: pięć barw kobiecych (F1–F5)
  i pięć męskich (M1–M5). Zmiana działa od następnego zdania, bez ponownego uruchamiania rozmowy.
- Domyślnie jak dotąd: Alfa F1, Beta F2, Gama F4, Delta F5. Agentki z Kreatora zostają przy głosie z łańcucha.

### Czego nie sprawdzono

- Brzmienia barw nie da się ocenić automatycznie; kliknij „Odsłuchaj” przy agentce po zmianie.
- Dwie agentki mogą dostać tę samą barwę — program na to pozwala i nie ostrzega.

### Testy

- Test łańcucha głosów: barwa z Ustawień trafia do silnika tylko dla agentek wbudowanych; test mapowania
  ustawień (nieznana nazwa barwy → barwa domyślna). vitest 311/311; testy okna 308 z 309 (znany próg strumienia).

## 0.0.30 — 2026-10-11

Nowy głos agentek: Supertonic 3. Cztery agentki mówią czterema różnymi, naturalniejszymi barwami — lokalnie, bez
Internetu i bez opłat. To pierwszy krok przenoszenia rozmowy i mowy z Delty AI (zadanie P1).

### Głos

- [poważne] Agentki mówią głosem Supertonic 3, gdy jego model jest pobrany: Alfa barwą F1, Beta F2, Gama F4,
  Delta F5 — naprawdę różne głosy zamiast jednego przestrajanego. Model (383 MB) pobierasz w Ustawieniach →
  „Modele lokalne i silniki”; bez niego wszystko działa jak dotąd na Piperze.
- Silnik działa jako osobny proces bez Pythona, na procesorze. Ustawienia przejęte z Delty: 6 kroków i tempo 1,05.
  Na tym laptopie zdanie jest gotowe po ok. 1 s (mediana 0,97 s; wcześniejsza próba w Pythonie: 2,10 s).
- Brak pobranego silnika mowy nie jest już zgłaszany jako awaria przy każdej wypowiedzi — program po prostu
  używa następnego głosu z listy.

### Naprawy

- Dane przykładowe podglądu pakietów modeli wróciły do postaci generowanej z rdzenia. Ręczna poprawka z 0.0.26
  („karta ma 6,0 GB”) rozjechała je z testem rdzenia, którego wtedy nie uruchomiłam.

### Czego nie sprawdzono

- Brzmienia po polsku nie da się ocenić automatycznie — odsłuch należy do Ciebie (Ustawienia → Głos → „Odsłuchaj”
  przy każdej agentce albo próbki w `alfa-glos-proba/wyniki/alfa-supertonic`).
- Rozmowy głosowej na żywo z nowym głosem, pobierania modelu przyciskiem w Ustawieniach i reguł licencji
  zależności (`cargo deny` działa tylko w CI) jeszcze nie sprawdzono.
- Biblioteka ONNX Runtime dla tego silnika nie jest jeszcze w katalogu pobrań — w tej budowie leży obok programu.

### Testy

- 11 testów nowego silnika (protokół, przygotowanie tekstu), 2 testy łańcucha głosów (barwa agentki i tempo,
  ciche pominięcie niezainstalowanego silnika), test katalogu (16 plików z sumami SHA-256).

### Przyczyna błędu

- Łańcuch głosów traktował silnik bez plików tak samo jak silnik, który się zepsuł: wysyłał zdarzenie przejścia na
  zapas i oznaczał głos jako pogorszony.

## 0.0.29 — 2026-10-11

Plan uzupełniony o pełną macierz funkcji Delty, trzy nowe grupy zadań i nowości z rynku. W samym programie ta
wersja niczego nie zmienia.

### Plan

- Macierz funkcji Delty pozycja po pozycji z decyzją dla każdej (`docs/plan/20`, `21`) i specyfikacja jej systemu
  rozmowy i mowy ze wszystkimi stałymi (`docs/plan/19`). Macierz porównana ze zrzutami menu od właściciela.
- Nowe grupy: agentki programistki z nadrzędnym przyciskiem „Pełne uprawnienia” (P12), dyski sieciowe i chmurowe
  z kopiami zapasowymi (P13), obowiązki codzienne agentek — poczta, kalendarz, przypomnienia (P14).
- Nowości z rynku dopisane do grup (`docs/plan/22`): rutyny, równoległe przebiegi agentek, tani model do czytania
  i mocny do pisania, sterowanie zadaniami głosem, poranny przegląd, zasady kopii i obrona poczty przed poleceniami
  ukrytymi w listach.

### Czego nie sprawdzono

- Nowości z rynku pochodzą częściowo ze źródeł wtórnych; każda będzie sprawdzona u źródła przed rozpoczęciem zadania.

## 0.0.28 — 2026-10-11

Jeden plan całości: dotychczasowe etapy, przeniesienie funkcji z Delty AI i wielu użytkowników. W samym programie
ta wersja niczego nie zmienia.

### Plan

- Plan zbiorczy w jednym dokumencie: stan prac, etapy A–H, cztery cele (Windows, Linux, panel WWW, Android)
  z pełnym sterowaniem i komunikacją — `docs/plan/16-plan-zbiorczy.md` (w przeglądarce: `docs/html/index.html`).
- Nowy etap P: przeniesienie z Delty AI 0.8.35 systemu rozmowy i mowy oraz brakujących funkcji w 12 grupach
  (głos, modele lokalne, nawigacja, rozmowa, sieć i wiedza, osoby, komputer, twórczość, awatar, multimedia,
  pozostałe) — `docs/plan/17-przeniesienie-z-delty.md`.
- Nowa grupa P12: agentki programistki — piszą, uruchamiają i budują oprogramowanie w katalogu projektu,
  rozmawiają ze sobą w trakcie pracy i dobierają model do trudności zadania (lekki do czytania, mocny dopiero
  gdy trzeba), z nadrzędnym przyciskiem „Pełne uprawnienia” jak w Claude Code i Codex.
- Nowe grupy P13 i P14: dyski sieciowe i chmurowe z kopiami zapasowymi oraz obowiązki codzienne agentek
  (poczta, kalendarz, przypomnienia).
- Nowy etap U: wielu użytkowników z kontami, logowaniem i rolami (Właściciel, Dorosły, Dziecko, Gość) na
  komputerze i zdalnie — `docs/plan/18-wielu-uzytkownikow.md`, projekt decyzji ADR 0027.
- Decyzje właściciela zapisane w planie: Alfa jest programem docelowym; ADR 0025 (dane biometryczne) przyjęty;
  głos Edge tylko jako opcja wyłączona domyślnie; stała zgoda na pobrania do 3 GB.

### Czego nie sprawdzono

- Analiza Delty powstała z czytania kodu, bez uruchamiania programu. Specyfikacja głosu (`docs/plan/19`) i macierz
  funkcji (`docs/plan/20`, `21`) są spisane z dwóch przejść po kodzie; jeśli przy pracy wyjdzie pominięta funkcja,
  macierz zostanie uzupełniona.

## 0.0.27 — 2026-10-11

Poprawki z przeglądu paneli bocznych i nakładek w pełnej skali.

### Interfejs

- [poważne] Skróty `Alt+5`, `Alt+6` i `Alt+7` otwierają teraz kartę stojącą na tym miejscu od lewej: karty panelu
  bocznego mają kolejność Agentki, Oś czasu, Pliki, Pamięć, Ekran, Głos, Zadania — taką samą jak skróty. Wcześniej
  `Alt+5` otwierał szóstą kartę, `Alt+6` siódmą, a `Alt+7` piątą.
- Pole „Wiadomość dla agentki” w panelu Zadania zajmuje całą szerokość karty zamiast wąskiego kawałka.
- Polecenie `/pamięć` w polu pisania ma opis „Otwórz panel Pamięć” (robi to od dawna; opis mówił „w kolejnej fali”).
- Ściąga skrótów: „pole pisania” zamiast „composer” i odstęp między treścią a ramką.

### Testy

- Przegląd paneli sprawdza teraz, że skrót z numerem N wybiera N-tą kartę (we wszystkich stylach i motywach).
- Nowy skrypt zrzutów widoku głównego, paneli i nakładek w skali 1:1: `scripts/shots-panels.mjs`.
- vitest 311/311; testy okna w przeglądarce 308 z 309 (znany próg strumienia).

### Przyczyna błędu

- Kolejność kart i numery skrótów były zapisane w dwóch miejscach i rozjechały się po dodaniu karty Zadania;
  reguła stylu pola wiadomości celowała w element, którego w tym miejscu nie ma.

## 0.0.26 — 2026-10-11

Poprawki z przeglądu stron Ustawień w pełnej skali i pierwszy pomiar silników mowy na tym laptopie.

### Interfejs

- [poważne] Przycisk „Rozwiń wszystkie” i wiersze wersji w Historii zmian mają zaokrąglone rogi jak reszta programu,
  a w menu pod prawym przyciskiem jest odstęp między nazwą polecenia a skrótem. Style odwoływały się do zmiennych,
  które nie istnieją, więc reguły po cichu nie działały.
- Nazwy zmiennych w oknie importu kluczy są pisane krojem o stałej szerokości.
- Prostsze teksty: „modele zapasowe” zamiast „fallbacki”, opis miniatur obrazów, „Limit dla każdego dostawcy ustawisz
  w „Konta i klucze API””, „L3 · Bardzo wysoka — poziom domyślny”.

### Głos (plan, bez zmian w programie)

- Zmierzone na tym laptopie trzy silniki mowy na 20 polskich zdaniach; strona do odsłuchu: `alfa-glos-proba/odsluch.html`
  (folder obok programu). Piper: 0,56 s na zdanie. Supertonic 3: 2,1 s. Chatterbox na karcie graficznej: 4,8 s
  i 3,6 GB pamięci karty.
- Wniosek: oba nowe silniki są w obecnej postaci za wolne do rozmowy; o dalszych krokach decyduje odsłuch.
  Szczegóły: `docs/plan/15-glos-lokalny.md`, część 5a.

### Testy

- Bramka `check-css` zgłasza teraz każdą zmienną stylu użytą bez definicji (sprawdzone na starej wersji pliku:
  zgłasza obie wady).
- vitest 311/311; testy okna w przeglądarce 308 z 309 (znany próg strumienia).

### Przyczyna błędu

- Literówka w nazwie zmiennej CSS nie daje błędu — przeglądarka pomija całą deklarację. Testy układu tego nie
  widziały, bo nic nie wystawało poza okno; wyszło dopiero przy oglądaniu zrzutów w skali 1:1.

## 0.0.25 — 2026-10-11

Plan lepszego głosu bez opłat za każde zdanie: zamiast głosu z chmury — dwa poziomy głosu lokalnego. W samym
programie ta wersja niczego nie zmienia; to dodatek do planu po analizie silników mowy i konkurencji.

### Plan

- Kierunek głosu zmieniony z chmury na lokalny: poziom „Naturalny” (Chatterbox Multilingual na karcie graficznej,
  cztery różne głosy agentek z krótkich próbek) i poziom „Szybki” na procesorze, przełączane automatycznie.
  Opis, porównanie silników, koszty chmury i kryteria wyboru: `docs/plan/15-glos-lokalny.md`.
- Nowa grupa zadań C5 w roadmapie (siedem zadań: licencje u źródła, stanowisko pomiarowe i odsłuch, silnik,
  głos szybki, automatyczny wybór, głosy wzorcowe, karty licencji).
- Głos z chmury (C1.1a) zostaje opcją wyłączoną domyślnie i schodzi na koniec kolejki — codzienna rozmowa
  najlepszymi głosami kosztowałaby kilkadziesiąt dolarów miesięcznie.
- Projekty decyzji 0024 i 0026 uzupełnione o wynik analizy; oba nadal czekają na podpis właściciela.

### Czego nie sprawdzono

- Żaden silnik nie został pobrany ani odsłuchany: jakość mowy po polsku i opóźnienie na karcie RTX 4050 są
  niezmierzone. Pierwszym zadaniem jest pomiar (C5.2) — wymaga zgody na pobranie ok. 1,5 GB.

## 0.0.24 — 2026-10-11

Przegląd każdego okna, panelu i skrótu — jako test, który powtarza się przy każdej zmianie — i poprawki z niego.

### Interfejs

- [poważne] `Ctrl+Shift+F` i przycisk „Szukaj” na pasku ikon sesji znów działają na oknie szerokości laptopa
  z otwartym prawym panelem: lista sesji rozwija się i fokus trafia do pola szukania. Wcześniej nie działo się nic.
- Okna programu mają ikonę strony (znak Alfy); podgląd w przeglądarce nie zgłasza już brakującej ikony.

### Przegląd

- Nowy przegląd układu przechodzi w trzech stylach i dwóch motywach: wszystkie 32 strony Ustawień (także w wąskim
  oknie), siedem paneli bocznych, nakładki otwierane skrótami, objaśnienia w górnej belce, podpowiedzi i menu pola
  pisania. Sprawdza, czy nic nie wystaje poza okno, czy żaden tekst nie jest ucięty, czy wszystko zamyka się
  klawiszem Escape i czy konsola jest bez błędów.
- Wynik i lista tego, czego przegląd jeszcze nie obejmuje: `docs/reviews/2026-10-11-przeglad-okien-i-skrotow.md`.

### Testy

- 19 nowych testów przeglądu (`e2e/review-sweep.spec.ts`); znalazły dwie wady opisane wyżej.
- Poza przeglądem zostają: ocena wizualna na oko, skróty globalne, okna Brokera, stany zależne od sprzętu i każdy
  krok kreatorów — zapisane jako następne kroki.

### Przyczyna błędu

- Polecenie szukania ustawiało „lista sesji otwarta”, ale na średnim oknie z prawym panelem lista jest zwinięta do
  paska ikon i pola szukania w ogóle nie ma na stronie — fokus trafiał w nic.

## 0.0.23 — 2026-10-11

Historia zmian w Aktualizacjach i spis funkcji oznaczonych „wkrótce”.

### Aktualizacje

- **Historia zmian:** Ustawienia → Aktualizacje pokazują teraz wszystkie wersje od najnowszej — co zostało zrobione
  w każdej. Okno „Co nowego” pojawia się jak dotąd raz, ale historia zostaje na stałe.
- Każda pozycja ma wagę: krytyczna (czerwona), poważna (pomarańczowa) albo zwykła (zielona) — pokazaną kolorem
  i słowem. Przy wersji widać, ile ma pozycji każdej wagi.
- Filtr „Krytyczne / Poważne / Zwykłe” zostawia tylko pozycje wybranej wagi; wersje można zwijać i rozwijać,
  także wszystkie naraz i z klawiatury.
- Wstecznie oznaczone zostały wszystkie dotychczasowe wersje: 12 pozycji krytycznych i 31 poważnych.
- Przycisk w oknie „Co nowego” prowadzi do historii zmian.

### Plan i porządek

- Spis funkcji oznaczonych „wkrótce” (`docs/plan/14-funkcje-wkrotce.md`): cztery strony Ustawień ze znakiem
  „wkrótce” (Router i reguły, Logi i prywatność, Moduły, Zaawansowane) dostały zadania w planie (grupa C4).
  Wcześniej istniały tylko w programie.
- W tym samym spisie są funkcje, które mówią „dostępna po podłączeniu modułu”, z wyjaśnieniem, kiedy ten
  komunikat widać, oraz funkcje zaplanowane, których program jeszcze nie pokazuje.

### Testy

- Historia zmian: testy rozbioru dziennika (wagi, sekcje, pomijane części), testy interfejsu w trzech stylach
  i dwóch motywach z kontrolą dostępności, zrzuty wszystkich stanów (`scripts/shots-history.mjs`).
- Nowy test pilnuje, żeby każda strona „wkrótce” była w spisie i miała zadanie w planie.

## 0.0.22 — 2026-10-11

Szkic zestawu do mierzenia jakości odpowiedzi po polsku. Bez zmian widocznych w programie (zadanie C2.1 planu).

### Pomiar jakości

- Nowy zestaw `evals/V6/pl-core` — 36 przypadków po polsku w trzech grupach: czy model wybiera właściwe narzędzie
  i podaje dobre argumenty (także nazwy plików z polskimi znakami), czy odpowiada zgodnie ze źródłem i przyznaje,
  gdy czegoś w nim nie ma, oraz czy nie wykonuje poleceń wstrzykniętych w dokument, stronę, e-mail albo wynik
  narzędzia (z dwoma przypadkami kontrolnymi fałszywych alarmów).
- Zestaw ma status „szkic”: czeka na Twoją akceptację i progi. Do tego czasu niczego nie blokuje.

### Testy

- Harness czyta zestaw i sprawdza jego integralność (hashe plików, klasy, podziały).
- Zestaw nie był jeszcze uruchomiony na żadnym modelu — brakuje wariantu uruchamiającego model z narzędziami;
  pomiary mowy (WER), czasu i pamięci karty wymagają sprzętu i są poza tym szkicem.

## 0.0.21 — 2026-10-11

Typy łącza między rdzeniem a oknem powstają teraz z kodu rdzenia — rozjazd wyłapuje kompilator, a nie użytkownik.
Bez zmian widocznych w programie (zadanie B.1 planu, etap pierwszy).

### Pod maską

- Z typów rdzenia powstaje schemat łącza (`packages/schemas/ipc.v1.json`: 225 poleceń, ramka zdarzeń, błąd,
  298 typów), a z niego plik typów dla okna. Zmiana typu w rdzeniu bez odświeżenia tych plików zatrzymuje testy.
- Kompilator sprawdza 501 zgodności typów okna z typami rdzenia oraz nazwy i argumenty wszystkich 224 wywołań
  poleceń — literówka w nazwie polecenia albo brak argumentu to teraz błąd budowania, a nie błąd w działającym
  programie.
- Przegląd przy okazji: typy pisane ręcznie zgadzały się z rdzeniem poza jednym polem (zakres pamięci w eksporcie
  paczki) — poprawione.

### Testy

- Schemat jest porównywany z kodem w testach rdzenia; plik typów — w bramce i w CI. Sprawdzone, że celowo zepsuty
  typ okna jest wyłapywany.
- Okno nadal używa typów pisanych ręcznie (teraz pilnowanych przez kompilator); ich usunięcie na rzecz
  generowanych to drugi etap zadania.

## 0.0.20 — 2026-10-11

Klucze API pod opieką: Alfa sama sprawdza je w tle, pokazuje wiek klucza i przełącza się na konto zapasowe.

### Konta i klucze API

- **Sprawdzanie kluczy w tle:** co 24 godziny (do zmiany w Ustawieniach → Konta i klucze API; 0 wyłącza) Alfa
  sprawdza każde włączone konto tym samym testem co przycisk „Testuj połączenie” — bez zużycia tokenów. Gdy klucz przestanie
  działać, dostaniesz komunikat, zanim zaczniesz rozmowę.
- **Wiek klucza:** przy każdym koncie widać, ile dni ma zapisany klucz; po 180 dniach (do zmiany, 0 wyłącza) pojawia
  się przypomnienie o wymianie. Alfa niczego nie wymienia sama.
- **Konto zapasowe:** gdy masz dwa konta u tego samego dostawcy, a pierwsze dostanie limit zapytań albo zacznie
  odrzucać zapytania (klucz, brak środków), kolejne wiadomości pójdą przez drugie — z komunikatem, bez Twojego
  udziału. Wiadomość, która trafiła na błąd, ponawiasz sam.
- Konto, które dostało limit zapytań, wraca na swoje miejsce, gdy limit minie (najpóźniej po godzinie).

### Testy

- Nowe testy całej ścieżki na lokalnych serwerach udających dostawcę: sprawdzanie w tle (klucz psuje się i wraca),
  wyłączone sprawdzanie i wyłączone konto bez zapytań, data klucza przy dodaniu i wymianie, przełączenie na konto
  zapasowe po limicie zapytań i po odrzuconym kluczu, brak przełączenia przy jednym koncie.
- Nie sprawdzono u prawdziwego dostawcy (wymaga dwóch prawdziwych kluczy tego samego dostawcy).
- Test `review.rs::queued_message_of_a_trashed_session_waits_for_undo` raz nie przeszedł w pełnym przebiegu pod
  obciążeniem i sześć razy z rzędu przeszedł osobno — przyczyna nieustalona, do obserwacji.

### Przyczyna błędu

- Router pamiętał limit zapytań dla dostawcy, a nie dla konta — drugie konto tego samego dostawcy było blokowane
  razem z pierwszym. Przy błędzie przed pierwszym fragmentem odpowiedzi nie było też wiadomo, które konto zawiodło.

## 0.0.19 — 2026-10-11

Pewniejsze łącze między rdzeniem a oknem: zdarzenia mają numery i dają się odtworzyć, a błędy niosą kod
i szczegóły. To pierwsze dwa „szwy” pod panel WWW i telefon (zadania B.2 i B.3 planu).

### Rozmowa

- [poważne] Odpowiedź pisana w chwili otwierania rozmowy nie gubi już ani nie powtarza fragmentów tekstu.
- [poważne] Gdy okno nie nadąży za rdzeniem (bardzo dużo zdarzeń naraz), samo dociąga brakujące zdarzenia zamiast po cichu
  je tracić; jeśli nie da się ich odtworzyć, wczytuje stan od nowa — bez zmiany widoku.

### Komunikaty błędów

- [poważne] W działającym programie błędy docierają do okna z kodem i szczegółami (czego dotyczą, które pole, po ilu
  sekundach ponowić), a nie jako sam tekst — okno może reagować na rodzaj błędu, a nie zgadywać z treści.
- [poważne] Szczegóły błędu nigdy nie niosą kluczy ani treści rozmów: przechodzą przez filtr, który odrzuca wszystko, co
  wygląda na sekret.

### Testy

- Nowe testy: bufor i numeracja zdarzeń (rdzeń), kolejność ramek po stronie okna (luka, utrata, inne uruchomienie
  rdzenia), fragmenty tekstu z czasu wczytywania, wczytanie stanu od nowa, filtr sekretów w szczegółach błędu,
  test prawdziwej aplikacji (błąd jako obiekt, polecenie `events_resume`).

### Przyczyna błędu

- Powłoka zamieniała każdy błąd rdzenia na napis, więc kod błędu działał tylko na atrapie interfejsu. Zdarzenia nie
  miały numerów, więc okno nie mogło wykryć zgubionej paczki ani odróżnić fragmentu tekstu, który już ma.

## 0.0.18 — 2026-10-10

Domknięcie otwartych ustaleń z dwóch przeglądów, które nie wymagają prawdziwej kamery.

### Kamery

- Znak działającej kamery jest też w małym oknie głosowym (pigułce) — widać go, gdy okno główne jest schowane.
- [poważne] Wyłączenie kamer nie przepada przez chwilowy błąd zapisu ustawień: zapis jest ponawiany.
- [poważne] Obraz z kamery jest dodatkowo sprawdzany pod kątem rozmiaru bufora, zanim program go skopiuje.

### Modele lokalne i głos

- [poważne] Pierwsze uruchomienie modelu na komputerze z kilkoma kartami nie wstrzymuje reszty programu na czas wykrywania kart.
- [poważne] Gdy brakuje pliku silnika, pamięć zarezerwowana dla modelu jest od razu zwalniana.
- Zapamiętane frazy mowy uwzględniają ustawienia głosu — po zmianie tempa albo głośności nie wróci stare nagranie.

### Testy

- Z 14 ustaleń obu przeglądów otwarte zostają 2 — oba wymagają prawdziwej kamery (zawieszony sterownik, kierunek
  wierszy obrazu). Kamera laptopa jest nadal odłączona sprzętowo (sprawdzone 2026-10-10 rano).

## 0.0.17 — 2026-10-10

„Opisz mi to, co widzisz”, poprawki po przeglądzie kodu kamer w świeżym kontekście i głośność mowy.

### Kamery

- **„Opisz mi to, co widzisz”:** gdy kamery są włączone i napiszesz taką prośbę (także „co widzisz na kamerze
  Kuchnia?”), Alfa dołączy zdjęcie z kamery — nazwanej w prośbie albo pierwszej włączonej — i pokaże je przy polu
  pisania. Wiadomość idzie dopiero po drugim Enterze, więc widzisz zdjęcie przed wysłaniem i możesz je usunąć.
  Zwykłe zdania („co widzisz w tym kodzie?”) zdjęcia nie robią. Można to wyłączyć w „Kamery”. Model musi
  przyjmować obrazy; w rozmowie głosowej jeszcze nie działa.
- [poważne] Znak kamery w górnej belce świeci już wtedy, gdy kamera się otwiera, i jeszcze wtedy, gdy się zamyka — nie tylko
  przy stanie „działa”. Kamera pokazuje nowy stan „zamykam…”, a „uśpiona” dopiero po faktycznym zamknięciu.
- [krytyczne] „STOP WSZYSTKIEGO” nie czeka już na kamery: zawieszona kamera nie opóźni zatrzymania agentek.
- [krytyczne] Przerwane żądanie obrazu nie zostawia kamery włączonej na stałe; podgląd kończy się sam, gdy kamera zgłosi błąd.
- Nazwa niedostępnej kamery jest czytelna także w jasnym motywie.

### Uruchamianie

- [poważne] Ekran ładowania nie może już zostać na zawsze: błąd startu jest na nim pokazywany razem z przyciskiem „Zamknij
  program”, a gdy start trwa ponad 20 sekund, pojawia się to samo wyjście. Górny pas ekranu ładowania przesuwa okno.

### Głos

- „Głośność mowy” w zakładce „Głos”: głośność głosu agentek niezależnie od głośności systemu (20–150 %).

### Testy

- Przegląd: `docs/reviews/2026-10-10-przeglad-kamer.md` — 9 ustaleń, 6 naprawionych, 3 otwarte (wymagają prawdziwej
  kamery: zawieszony sterownik, kierunek wierszy obrazu, ponawianie zapisu wyłączenia).
- Drugi przegląd (zmiany 0.0.11–0.0.17): `docs/reviews/2026-10-10-przeglad-0.0.11-0.0.17.md` — 5 ustaleń, 3
  naprawione (dwa dotyczyły prywatności zdjęcia z kamery), 1 częściowo, 1 otwarte.
- Nadal nie sprawdzono na prawdziwej kamerze (kamera laptopa odłączona sprzętowo).

## 0.0.16 — 2026-10-10

Praca nocna bez właściciela: pierwszy krok systemu kamer, import kluczy ze zmiennych środowiskowych, porządki
w interfejsie i pakiet decyzji do podjęcia.

### Kamery (pierwszy krok planu C3)

- W Ustawieniach jest strona „Kamery”: lista kamer wbudowanych i na USB, nazwy do zmiany, przełącznik każdej kamery
  i przełącznik główny. Kamery są domyślnie wyłączone.
- Podgląd na żywo z kilku kamer naraz. Kamera otwiera się dopiero na żądanie i sama zasypia po 20 sekundach bez
  podglądu; podgląd niczego nie zapisuje.
- Gdy którakolwiek kamera działa, w górnej belce świeci czerwony znak kamery z liczbą działających kamer.
- W polu pisania jest przycisk z aparatem: dołącza zdjęcie z kamery do wiadomości — można wtedy napisać „opisz mi
  to, co widzisz”. Potrzebny jest model, który przyjmuje obrazy.
- [poważne] „STOP WSZYSTKIEGO” zamyka także kamery.

### Konta i klucze

- „Importuj ze zmiennych środowiskowych” na stronie kont: Alfa sprawdza nazwy z katalogu dostawców (np.
  `OPENAI_API_KEY`), znaleziony klucz zapisuje w Windows Credential Manager i pokazuje raport z samymi nazwami zmiennych.

### Interfejs

- Panel „Agentki” jest zwarty: role i „Zmień role” w jednym wierszu, jedna podpowiedź na dole zamiast czterech —
  cała obsada mieści się bez przewijania.
- Strona „Urządzenia” (procesor, karty, bateria) przeniesiona z grupy głosu do grupy „System”; grupa głosu nazywa się
  teraz „Głos i kamery”.

### Dokumenty

- Pakiet decyzji właściciela: 44 otwarte decyzje bezpieczeństwa i podpisy pod projektami ADR, każda z opisem po
  ludzku, opcjami i rekomendacją (`docs/reviews/2026-10-10-pakiet-decyzji-wlasciciela.md`).
- Podsumowanie nocy i plan dalszych prac: `docs/plan/13-podsumowanie-i-dalsze-kroki.md`.

### Testy

- Nowe: `e2e/cameras.spec.ts`, `e2e/env-import.spec.ts`, testy huba kamer i portu kamery w Rust, test importu kluczy.
- **Nie sprawdzono na prawdziwej kamerze.** Windows zgłasza kamerę laptopa jako odłączoną sprzętowo (zasłonka albo
  klawisz funkcyjny), więc kod przechwytywania obrazu działał tylko na atrapach. Po odsłonięciu kamery trzeba to
  sprawdzić: „Kamery” → włącz → „Szukaj kamer” → „Podgląd”.
- Import kluczy z prawdziwych zmiennych środowiskowych nie był uruchamiany (sesja nie dotyka kluczy właściciela).

## 0.0.15 — 2026-10-09

Zlecenie właściciela: dopisać do planu prac system kamer, rozpoznawanie użytkowników z pamięcią i opis obrazu.

### Plan

- W planie jest nowa grupa zadań „Kamery, rozpoznawanie osób i opis obrazu” (C3, 15 zadań): kamery USB, IP i Wi-Fi,
  kilka naraz, przez osobny system kamer z podglądem, przełącznikami i wskaźnikiem w belce.
- Zaplanowane jest polecenie „opisz mi to, co widzisz”: Alfa bierze klatkę z kamery i opisuje ją modelem lokalnym
  albo — za osobną zgodą — modelem w chmurze.
- Zaplanowane jest rozpoznawanie zapisanych osób po twarzy i pamięć długoterminowa każdej osoby: kim jest i o czym
  Alfa z nią rozmawiała, także po ponownym uruchomieniu programu.
- Zasady prywatności są spisane jako projekt decyzji (ADR 0025): kamery domyślnie wyłączone, bez nagrywania, wzorce
  twarzy tylko za zgodą osoby i tylko na komputerze, a rozpoznanie twarzy nigdy nie zastępuje zgody w Brokerze.
- To jest plan — w programie nic się jeszcze nie zmieniło. Kamery i opis obrazu mogą ruszyć od razu; rozpoznawanie
  osób czeka na podpis pod ADR 0025.

### Dokumenty

- Nowe: `docs/plan/12-kamery-i-rozpoznawanie-osob.md`, szkice SPEC `camera-hub`, `vision-describe`,
  `person-recognition`, `person-memory`, projekt `docs/ADR/0025-kamery-i-biometria-twarzy.md`.
- Uzupełnione: roadmapa (C3 i decyzje blokujące), katalog funkcji, zgodność prawna, przegląd technologii (rozdział 10 —
  bez przeglądu źródeł, wszystko oznaczone jako niesprawdzone).

## 0.0.14 — 2026-10-09

Zgłoszenia właściciela: pasek LED wchodzi na elementy belki, brak ekranu ładowania, ikony agentek do solidnej
poprawy (mruganie, mini awatary w rytm mowy), głos agentek sztywny i niemiły.

### Interfejs

- Belka jest wyższa (44 px), a pasek LED ma pod elementami własny tor — nic na niego nie wchodzi i między kapsułami
  a paskiem jest wyraźny odstęp. Kapsuły, awatary i przyciski mają wspólną wysokość i równe odstępy.
- Program ma ekran ładowania: znak Alfy, nazwa, pasek postępu i opis kroku („Uruchamiam rdzeń…”, „Wczytuję
  rozmowy…”). Widać go od pierwszej klatki okna, w kolorach zapamiętanego motywu.
- Awatary agentek ożyły: agentka, która mówi albo pracuje, dostaje twarz — mruga, mówiąc rusza ustami, pracując
  rozgląda się, a wokół krąży łuk. Gotowa agentka zostaje literą. Duże awatary mają twarz zawsze.
- Usta poruszają się w rytm prawdziwej mowy: program mierzy poziom tego, co gra głośnik, i wysyła go do awatara.
- W „Wygląd” jest przełącznik „Awatary agentek”: „Żywe” albo „Same litery”.

### Głos

- Głos agentek brzmi naturalniej: silnik mowy dostaje żywszą intonację i rytm zamiast ustawień surowych, a po każdym
  zdaniu jest krótka pauza — mowa nie idzie jednym ciągiem.
- Tempo mowy ustawia sam silnik, a nie rozciąganie gotowego nagrania, więc szybsza albo wolniejsza mowa nie brzmi
  jak przyspieszona taśma.
- Różnice między głosami agentek są domyślnie delikatniejsze — mocne przestrajanie jednego głosu na cztery było
  głównym źródłem sztucznego brzmienia.
- Zakładka „Głos” ma nową część „Głosy agentek”: styl mówienia (spokojny, naturalny, żywy), tempo, pauza między
  zdaniami i siła różnic między głosami. Zmiana działa od następnego zdania; obok jest odsłuch każdej agentki.

### Testy

- Nowe: `e2e/presence.spec.ts` (ekran ładowania, tor paska LED w 3 stylach × 2 motywach, awatary, strojenie mowy
  i odsłuch), `boot-screen.test.ts`, testy strojenia Pipera, miernika poziomu mowy i ustawień głosu w Rust.
- Nie sprawdzono na żywo: brzmienia po zmianach (ocena na słuch należy do właściciela) i ruchu ust przy prawdziwej
  mowie — sprawdzone na danych testowych. Wszystkie agentki nadal mówią jednym głosem Pipera („Gosia”); wyraźnie
  lepsze, różne głosy wymagają innego silnika mowy (pobranie albo usługa w chmurze — decyzja właściciela).

## 0.0.13 — 2026-10-09

Zgłoszenie właściciela: belka okna z ikoną i nazwą „Alfa” — „ten element też dopracuj”.

### Interfejs

- Alfa ma własny znak: zaokrąglony kwadrat z przejściem fiolet → błękit i białą literą α. Zastępuje pusty fioletowy
  krążek jako ikona programu — na pasku zadań, w zasobniku i w przełączniku okien.
- Belka okna zaczyna się od znaku i nazwy „Alfa”, jak belka systemowa, ale w stylu programu; za znak też można
  przeciągać okno. W wąskim oknie zostaje sam znak, w bardzo wąskim ustępuje miejsca przyciskom.
- Belki systemowej nie ma także po maksymalizacji i po przywróceniu okna z paska zadań (sprawdzone na działającym
  programie).

### Testy

- `e2e/chrome.spec.ts`: znak i nazwa na belce w 3 stylach × 2 motywach; `e2e-app`: brak belki systemowej także w
  oknie zmaksymalizowanym.

## 0.0.12 — 2026-10-09

Zgłoszenie właściciela: obsada agentek, mikrofon i stan w górnej belce „też popraw solidnie”.

### Interfejs

- Obsada agentek jest jedną kapsułą: gotowa agentka jest przygaszona, pracująca ma pierścień i kropkę, mówiąca
  pulsuje, a czekająca na zatwierdzenie dostaje bursztynowy znacznik. Podpowiedź podaje imię, rolę, stan i to, co
  agentka właśnie robi.
- Mikrofon to wyraźny przycisk stanu: włączony ma zieloną obwódkę i słupki poziomu, które ruszają się, gdy Alfa Cię
  słyszy; wyciszony — bursztynową; niedostępny — przerywaną z wykrzyknikiem.
- Stan („Hybryda · L2 · 0,00 zł”) to teraz kapsuła z dwiema częściami i ikonami: połączenie, profil modeli i poziom
  autonomii oraz osobno koszt sesji.
- Klik w profil i autonomię otwiera objaśnienie po ludzku — co znaczy profil, co wolno agentkom na tym poziomie, czy
  jest sieć i ile wiadomości czeka — z przejściem do „Uprawnienia” i „Konta i klucze API”.
- Klik w koszt otwiera szczegóły kosztów; przy 80 % limitu miesięcznego koszt jest oznaczony ostrzeżeniem.
- Bez sieci zamiast zielonej kropki jest znak braku połączenia. W bardzo wąskim oknie belka zostawia tylko
  najważniejsze elementy i nic nie wychodzi poza okno.

### Testy

- `e2e/chrome.spec.ts`: obsada, mikrofon, objaśnienie stanu i koszty w 3 stylach × 2 motywach, stan bez sieci;
  zrzuty wszystkich stanów: `scripts/shots-bar.mjs`.

## 0.0.11 — 2026-10-09

Zgłoszenia właściciela: górna belka z jadącym paskiem LED, mocno dopracowana nawigacja Ustawień i współpraca kilku
kart graficznych.

### Interfejs

- Górna belka okna jest teraz belką Alfy: systemowy pasek „Alfa” zniknął, a minimalizuj, maksymalizuj i zamknij
  rysuje program — w każdym stylu i motywie.
- Wzdłuż belki jedzie od lewej do prawej pasek LED w barwach agentek; przyspiesza, gdy agentka pracuje albo mówi,
  robi się bursztynowy bez sieci, a przy wyłączonych animacjach stoi jako równa listwa.
- Nawigacja Ustawień od nowa: każda strona ma ikonę, bieżąca — znacznik przy krawędzi, grupy można zwijać (program
  to pamięta), nagłówek z wyszukiwarką stoi w miejscu, a przewija się tylko lista.
- Szukanie ustawień pokazuje w nawigacji tylko strony z trafieniami i ich liczbę; `Ctrl+F` przenosi do pola, krzyżyk
  albo `Esc` czyści, strzałki, `Home` i `End` chodzą po liście, `Enter` otwiera pierwszą stronę.
- W wąskim oknie lista stron chowa się pod przyciskiem z nazwą bieżącej strony, więc treść strony ma całe okno.

### Karty graficzne

- Alfa rozpoznaje każdą kartę osobno i nadaje jej rolę: mocna karta dostaje model rozmowy i rozpoznawanie mowy,
  zintegrowana — lekkie zadania w tle (streszczenia, tytuły, porządki pamięci). Role widać w „Urządzenia”.
- Model rozmowy jest przypinany do karty głównej, więc na laptopie z dwiema kartami nie trafi przypadkiem na słabszą.
- Lekkie zadania na karcie zintegrowanej wymagają silnika „llama.cpp Vulkan” — bez niego idą jak dotąd; po pobraniu
  silnika w „Modele lokalne i silniki” zaczynają działać same. Rozpoznawanie mowy zostaje na karcie głównej
  (silnik mowy nie ma wersji dla kart zintegrowanych).

### Testy

- Nowe: `e2e/chrome.spec.ts` (belka, pasek LED, przyciski okna i nawigacja w 3 stylach × 2 motywach, klawiatura,
  wąskie okno, role kart), `settings-nav.test.ts`, testy przypinania do karty (`devices.rs`), lekkiego dostawcy
  w Routerze i wykrywania kart.
- Nie sprawdzono na sprzęcie: praca modelu pomocniczego na karcie zintegrowanej (na laptopie właściciela nie ma
  silnika Vulkan — pobranie wymaga zgody). Przyciąganie okna do układów Windows po najechaniu na „maksymalizuj”
  (Snap Layouts) nie działa przy własnej belce; skróty `Win+strzałki` i przeciąganie do krawędzi działają.

## 0.0.10 — 2026-10-09

Zgłoszenia właściciela: systemowe menu po kliknięciu prawym przyciskiem, niewygodne przestawianie modeli strzałkami
i słaba organizacja ustawień modeli.

### Interfejs

- Menu po kliknięciu prawym przyciskiem rysuje Alfa (Cofnij, Wytnij, Kopiuj, Wklej, Zaznacz wszystko) — systemowe
  menu z „Emoji”, „Kierunek pisania” i „Wykonaj inspekcję” nie pojawia się w żadnym oknie.
- „Wklej” z menu czyta schowek przez rdzeń programu — WebView2 odmawia stronie dostępu do schowka (wyszło w teście
  prawdziwej aplikacji).
- Podpowiedzi przycisków rysuje Alfa zamiast systemowych dymków.
- Znikają pozostałe elementy rysowane przez system: strzałki pól liczbowych, krzyżyk i „oko” w polach, trójkąt
  rozwijania sekcji, uchwyt zmiany rozmiaru pola; pola nie podpowiadają wcześniejszych wpisów listą systemu.
- Kolejka modeli to kafelki do przenoszenia: podnieś kafelek i upuść w nowym miejscu. Podniesiony kafelek jest
  wyraźnie oznaczony, a po upuszczeniu albo dodaniu przez chwilę świeci. Z klawiatury: uchwyt, Spacja, strzałki.
- Ustawienia modeli mają jaśniejsze nazwy: „Konta i klucze API”, „Kolejki modeli API”, „Mosty CLI” (osobna strona),
  „Modele lokalne i silniki”.

### Testy

- E2E: 12 testów menu kontekstowego i podpowiedzi (6 kombinacji stylu i motywu), przenoszenie kafelków wskaźnikiem
  i z klawiatury; test w prawdziwej aplikacji: menu Alfy i wklejanie ze schowka systemu.

## 0.0.9 — 2026-10-09

Pierwszy krok porządkowania interfejsu po zgłoszeniu właściciela („organizacja”).

### Interfejs

- Przy każdym koncie w „Modele i dostawcy” jest przycisk „Modele i kolejki” — otwiera stronę kolejek od razu z tym
  kontem; ze strony kolejek jednym kliknięciem wracasz do klucza, testu i limitu konta.
- Szczegóły pod odpowiedzią (model, tokeny, koszt, czas) można wyłączyć w Ustawieniach → Wygląd.

### Testy

- E2E: przejście z listy kont do kolejek i z powrotem, przełącznik szczegółów odpowiedzi; poprawiony test dymny listy
  stylu w prawdziwej aplikacji (sprzątanie zamykało Ustawienia).

## 0.0.8 — 2026-10-09

Pięć zgłoszeń właściciela z pierwszej próby wersji 0.0.7 na laptopie: rozmowa z kluczem OpenAI nie działała, nie dało
się wybrać modeli, mikrofon zbierał szum, rozmowa głosowa słuchała bez odpowiedzi, a model lokalny dopisywał do
odpowiedzi techniczny tekst.

### Rozmowa i modele

- [krytyczne] Rozmowa z kontem OpenAI działa: Alfa wybiera model rozmowy, a nie pierwszy z listy dostawcy. Wcześniej trafiała na
  `tts-1` (synteza mowy) i każda wiadomość kończyła się błędem „The model tts-1 does not exist”.
- Nowa strona „API i kolejki modeli” w Ustawieniach. Dla każdego konta API ustawiasz kolejkę do dziesięciu modeli
  osobno dla rozmowy, kodu, planowania, szybkiej odpowiedzi głosowej, ekranu i obrazu, streszczania i embeddingów.
- Alfa bierze pierwszy model z kolejki, a następny dopiero wtedy, gdy poprzedni zawiedzie — także gdy dostawca
  odpowie, że modelu nie ma albo konto nie ma do niego dostępu.
- Przeglądarka modeli konta: wyszukiwarka, filtry rodzaju (rozmowa, rozumowanie, embeddingi, mowa, obrazy…),
  ukrywanie wersji z datą w nazwie. Każdy model ma opis, do czego służy.
- „Dobierz automatycznie” układa kolejkę jednym kliknięciem: najlepsza jakość, zrównoważone albo oszczędnie i szybko.
- Model, który do zadania się nie nadaje (np. synteza mowy w rozmowie), nie wejdzie do kolejki — z wyjaśnieniem.
- Agentki w roli programistki korzystają z kolejki „Kod”, planujące i recenzujące — z kolejki „Planowanie”.
- [poważne] Model lokalny nie dopisuje już do odpowiedzi zmyślonych wywołań narzędzi w rodzaju `{"name": "get_status", …}`.

### Głos

- [krytyczne] Rozmowa głosowa odpowiada. Wcześniej mikrofon słuchał, ale koniec wypowiedzi nigdy nie był wykrywany, więc nic nie
  trafiało do rozpoznawania mowy ani do modelu.
- Redukcja szumów mikrofonu ma trzy poziomy — wyłączona, standardowa i mocna (Ustawienia → Głos). Poza usuwaniem
  szumu działa filtr dudnienia wentylatorów i bramka, która ścisza tło między wypowiedziami.
- [poważne] Szum wentylatora nie jest już brany za mowę, więc nie przedłuża wypowiedzi i nie budzi rozpoznawania.
- Test mikrofonu pokazuje dwa paski: sygnał po redukcji szumów i surowy sygnał z mikrofonu — widać, ile tła odcięto.

### Testy

- Rdzeń: test całej ścieżki z lokalnym serwerem udającym OpenAI (lista z `tts-1` na początku → rozmowa idzie do
  modelu rozmowy; kolejka; pominięcie niedostępnego modelu) — `app-core/tests/accounts_models.rs`; reguły kolejek
  i rozpoznawanie przeznaczenia modeli — `accounts-hub-contract` (`model_use.rs`, `queue.rs`).
- Głos: zegar potoku sprawdzony na prawdziwym mikrofonie laptopa (`app-core/tests/voice_clock.rs`); szum wentylatora
  na trzech poziomach redukcji i zachowanie mowy (`voice-dsp-impl/tests/module.rs`); filtr i bramka (`gate.rs`).
- Model lokalny: odpowiedź z laptopa pocięta w każdym miejscu (`providers-local-impl/src/leak.rs`).
- Interfejs: 15 testów E2E strony API i testu mikrofonu (6 kombinacji stylu i motywu, wersja angielska), 8 testów
  logiki kolejek; zrzuty wszystkich stanów obejrzane ręcznie (`scripts/shots-api.mjs`).

### Przyczyna błędu

- Rozmowa: Router brał pierwszy model z listy konta; OpenAI zwraca listę alfabetycznie zmieszaną z modelami mowy,
  obrazów i embeddingów.
- Głos: znaczniki czasu ramek mikrofonu są w zegarze systemu (od jego startu), a potok liczył „teraz” od własnego
  startu — cisza po wypowiedzi wychodziła zawsze zerowa. Na atrapach w testach oba zegary były zgodne.
- Szum: redukcja szumu uznawała buczenie wentylatora za mowę, a test mikrofonu pokazywał sygnał przed redukcją.

## 0.0.7 — 2026-10-09

Dodawanie klucza API zrobione od nowa po zgłoszeniu właściciela („401 Klucz odrzucony” bez wyjaśnienia) i po dwóch
przeglądach całego systemu kluczy w świeżym kontekście; trzeci przegląd (19 ustaleń) naprawiony przed wydaniem —
raport: `docs/reviews/2026-10-09-przeglad-kluczy-api.md`.

### Klucze API i konta dostawców

- Wynik sprawdzenia klucza mówi, co się stało i co zrobić — osobno dla klucza odrzuconego, braku środków na koncie,
  ograniczonych uprawnień, kraju nieobsługiwanego przez dostawcę, odmowy z innego powodu, braku odpowiedzi i braku
  połączenia. Pod spodem można rozwinąć dokładną odpowiedź dostawcy (kod HTTP, kod błędu, komunikat).
- Kreator pokazuje, skąd wziąć klucz (adres strony z przyciskiem kopiowania) i uprzedza, że API jest płatne osobno od
  abonamentu czatu.
- Wklejony klucz jest sprzątany z tego, co dokleja schowek: cudzysłowów, `OPENAI_API_KEY=`, `export`, `set`, `Bearer`,
  przecinka, znaków nowej linii i znaków niewidocznych.
- Zdanie zamiast klucza, klucz innego dostawcy albo token logowania Claude Code są rozpoznawane, zanim cokolwiek
  zostanie wysłane.
- [poważne] Klucz bez prawa do listy modeli działa po wpisaniu nazwy modelu — wcześniej takie konto było bezużyteczne.
- „Zmień klucz” przy koncie: nowy klucz jest najpierw sprawdzany, a poprzedni zostaje, dopóki dostawca nie rozpozna
  nowego. Limit zapytań, błąd sieci ani odmowa nie zastępują działającego klucza. Ustawienia konta (przypisania,
  limit) nie giną.
- [poważne] Zły klucz u Google i xAI (zgłaszany kodem 400) jest pokazywany jako zły klucz, a nie „błąd połączenia”.
- OpenRouter i własne adresy bez listy modeli: Alfa mówi wprost, że klucza nie da się sprawdzić z góry, i prosi
  o nazwę modelu — zamiast udawać, że test się udał.
- [poważne] Dwie linie pliku `.env` wklejone naraz nie są już sklejane w jeden „klucz”.
- Konto można wyłączyć i włączyć bez usuwania klucza; włączenie przywraca stan z ostatniego testu.
- Lista kont pokazuje powód ostatniego testu („brak środków na koncie”, „dostęp odmówiony”) i przycisk „Co zrobić”.
- Ten sam klucz dodany drugi raz daje czytelną odmowę z nazwą istniejącego konta.
- Mistral, DeepSeek, OpenRouter i Google nie pytają już o adres API — jest w katalogu.
- [krytyczne] Zmiana dostawcy w kreatorze czyści wpisany klucz i adres, żeby klucz nie poszedł na serwer innego dostawcy.
- Gdy klucz może być dobry (brak środków, sieć, odmowa), konto można zostawić i sprawdzić ponownie później.

### Bezpieczeństwo

- [poważne] Odpowiedź dostawcy pokazywana w interfejsie ma zamaskowane wszystko, co wygląda jak klucz, i usunięte znaki
  sterujące; jest obcinana do 400 znaków.
- [poważne] Rdzeń odrzuca tekst, który po sprzątaniu ma spacje albo znaki spoza ASCII — nie trafia on do Menedżera poświadczeń.
- [krytyczne] Adres własnego endpointu z danymi logowania przed `@` (np. `http://localhost:80@inny.host`) jest odrzucany.
- [krytyczne] Połączenia z dostawcami nie podążają za przekierowaniami, żeby klucz nie trafił pod inny adres.
- [krytyczne] Test i wymiana klucza nie wysyłają go do dostawcy, który został oznaczony jako zabroniony.

### Testy

- Rdzeń: sześć testów całej ścieżki z lokalnym serwerem udającym dostawcę (`app-core/tests/accounts_keys.rs`,
  `accounts_rotate.rs`), klasyfikacja odpowiedzi (`app-modules/src/probe.rs`), wspólna tabela wklejanych kluczy dla
  Rust i TypeScript (`accounts-hub-contract/tests/fixtures/key-inputs.json`).
- Interfejs: 24 testy E2E kreatora i listy kont (każdy wynik, sześć kombinacji stylu i motywu z kontrolą kontrastu,
  wersja angielska, brak zdublowanych identyfikatorów), 17 testów logiki klucza.
- Zrzuty wszystkich stanów w sześciu kombinacjach stylu i motywu obejrzane ręcznie (`scripts/shots-wizard.mjs`).

### Przyczyna błędu

- Polecenie `accounts_test` zamieniało każdą odmowę uwierzytelnienia na stały tekst „Klucz odrzucony (401)” i gubiło
  odpowiedź dostawcy, więc brak środków, ograniczone uprawnienia i blokada regionu wyglądały jak zły klucz.

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
- [poważne] Dwa równoległe wczytania tej samej rozmowy nie gubią już zdarzenia „Stop”; usunięcie aktywnej sesji otwiera następną
  raz, nie dwa.
- [poważne] Przycisk „Uruchom ponownie” po aktualizacji pyta rdzeń o blokadę (trwająca rozmowa głosowa, zadanie agentki) — nie
  zostaje wyłączony po jej ustaniu ani włączony wbrew stanowi.
- Prawy panel przeciągnięty dokładnie na szerokość domyślną nie przeskakuje na szerokim oknie.

### Narzędzia agentek i rdzeń

- [poważne] Wyjście PowerShella ucięte limitem w środku znaku albo z pojedynczym obcym bajtem zostaje czytelne — wcześniej całe
  zamieniało się w krzaczki.
- [poważne] Polecenia PowerShella zaczynające się od `using`, `param` albo `#requires` znów działają (dopisywany przedrostek
  kodowania robił z nich błąd składni).
- [poważne] Kolejka offline: sesja, której odpowiedź nie może wystartować, nie blokuje pozostałych; jej wiadomość zostaje w kolejce
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

- [krytyczne] Przy skali powyżej 100 % (każde okno od 1920 px) strona była wyższa niż okno i pole pisania lądowało pod dolną
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
- [poważne] „Edytuj i wyślij ponownie” zachowuje załączniki; nieudany start odpowiedzi nie dubluje wiadomości.
- [krytyczne] Kolejka offline nie gubi wiadomości przy błędzie jednej sesji i nie wysyła wiadomości z sesji leżącej w koszu.
- [poważne] Sesja usunięta poza oknem nie zostaje „aktywna”; błąd w jednym zdarzeniu nie blokuje tury w stanie „pisze”;
  zamknięcie okna nie gubi szkicu.

### Głos

- [poważne] „Przeczytaj na głos” zawsze odmawiało (aplikacja nie miała dostępu do schowka) — naprawione.
- Czytanie i dyktowanie bez silnika lub modelu mowy mówią, czego brakuje i gdzie to pobrać.
- [poważne] STOP WSZYSTKIEGO kończy także dyktowanie.

### Narzędzia agentek

- [krytyczne] Odczyt pliku wczytuje tylko żądany fragment (duży plik nie zabija procesu); limit zapisu obejmuje plik po dopisaniu.
- [poważne] Nazwy urządzeń Windows (`nul`, `con`, `com1`…) są odrzucane jako ścieżki.
- Wyjście poleceń PowerShell i `cmd` zachowuje polskie znaki.
- [poważne] Pobierane strony są dekodowane według kodowania serwera; za długie przekierowanie jest odmową.
- [poważne] Cofnięcie zmian po restarcie aplikacji usuwa też utworzone katalogi; nieudane „Cofnij” schowka można powtórzyć.

### Modele, kopie i import

- [poważne] Nieudany start modelu nie zostawia „zajętej” pamięci; duże modele mają dłuższy limit czasu ładowania.
- [krytyczne] Kopia zapasowa nie powstaje w wersji nieszyfrowanej, gdy nie da się odczytać hasła.
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
