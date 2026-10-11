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
