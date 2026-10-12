# Raport z nocy 2026-10-11 / 12 (wersje 0.0.34 – 0.0.44)

Praca od 22:18 do rana, bez udziału właściciela. Jedenaście wersji; ostatnia zbudowana i sprawdzona to **0.0.44**.
Ten dokument mówi, co doszło, co jest sprawdzone i jak, czego nie sprawdziłem oraz co czeka na decyzję.

## 1. Najważniejsze w czterech punktach

- **Rozmowa głosowa ma komplet lokalnych silników i przeszła próbę na prawdziwych silnikach tego laptopa**: nagranie
  podane zamiast mikrofonu zostało rozpoznane bezbłędnie, a odpowiedź wypowiedziana głosem Supertonic; od końca mowy
  do przekazania tekstu mija ok. 0,2 s, odpowiedź słychać po ok. 1 s (bez czasu myślenia modelu rozmowy).
- **Żywej mowy człowieka do mikrofonu nie sprawdzałem** — to wymaga Ciebie przy komputerze. Sprawdziłem po cichu, że
  potok startuje na prawdziwym mikrofonie laptopa, słyszy otoczenie i nie przepuszcza szumu do rozmowy.
- **W Ustawieniach → Głos jest przycisk „Sprawdź silniki”** — jednym kliknięciem zobaczysz, czy rozpoznawanie,
  wyczucie końca wypowiedzi i głos działają u Ciebie, z czasem każdego z nich.
- **Gotowe obowiązki codzienne agentek dodałem i tej samej nocy ukryłem**: próba na lokalnym modelu pokazała, że
  agentka pisze raport z wymyślonymi zadaniami i kwotami, bo nie dostaje prawdziwych danych. Mechanizm działa;
  wzory wrócą, gdy program będzie podawał jej fakty.

## 2. Co doszło — wersja po wersji

| Wersja | Co doszło dla użytkownika | Grupa planu |
| --- | --- | --- |
| 0.0.34 | Alfa słyszy po brzmieniu, czy skończyłeś mówić (model Smart Turn v3.2); model i biblioteka ONNX Runtime w katalogu pobrań | P1 |
| 0.0.35 | Karta „Silniki rozmowy” ze stanem strażnika i przyciskiem „Zamknij bezpiecznik”; cztery ustawienia rozmowy; silniki rozgrzewają się przy włączeniu mikrofonu; nowe silniki głosu w pakietach jednym kliknięciem | P1, P2 |
| 0.0.36 | „Karty graficzne na żywo” na stronie Urządzenia: pamięć, obciążenie, temperatura, moc, programy zajmujące kartę | P2 |
| 0.0.37 | „Wstecz / Dalej” po widokach (Alt+←, Alt+→, boczne przyciski myszy); liczniki pracy Smart Turn | P3, P1 |
| 0.0.38 | Rozpoznawanie mowy dobiera silnik do zajętości karty graficznej; mały Whisper jako zapas na procesor | P1 |
| 0.0.39 | Ocena „mieści się na karcie” przy modelach rozmowy; próba potoku na prawdziwym mikrofonie | P2, P1 |
| 0.0.40 | Cztery gotowe obowiązki codzienne agentek (przegląd dnia, podsumowanie, porządki, koszty) — ukryte w 0.0.44 | P14 |
| 0.0.41 | „Sprawdź silniki” — próba każdego silnika rozmowy bez mikrofonu i bez dźwięku | P1 |
| 0.0.42 | Język rozpoznawania mowy w ustawieniach (domyślnie polski); wyszukiwarka w ściągawce skrótów | P1, P3 |
| 0.0.43 | Zwarta karta silników w bocznym panelu „Głos”; test płynności strumienia doprecyzowany | P1 |
| 0.0.44 | Gotowe obowiązki ukryte po próbie na lokalnym modelu (raport z wymyślonymi danymi) | P14 |

Szczegóły każdej wersji: [`CHANGELOG.md`](../../CHANGELOG.md).

## 3. Pomiary z tego laptopa (RTX 4050 6 GB)

| Co | Wynik |
| --- | --- |
| Smart Turn — ocena końca wypowiedzi | 21–46 ms; zgodność z wzorcem wydawcy do czwartego miejsca po przecinku |
| Qwen3-ASR na karcie — zdanie 4 s | 0,26 s (zimny start 2,4 s) |
| Qwen3-ASR na procesorze — to samo zdanie | 2,4 s (zimny start 6,2 s) |
| Whisper turbo na procesorze — to samo zdanie | 54 s — dlatego zapas na procesorze bierze mały model |
| Rozmowa „na sucho”: od końca mowy do przekazania tekstu | 0,13–0,18 s; pierwsza wypowiedź po włączeniu mikrofonu do 3,5 s, gdy silnik jeszcze się wczytuje |
| Rozmowa „na sucho”: od tekstu do słyszalnej odpowiedzi | 1,0–1,2 s (pierwsza 1,6 s) |
| „Sprawdź silniki” | rozpoznawanie 2,5 s, koniec wypowiedzi 0,2 s, głos 1,9 s |

Tabele ze szczegółami: [`19-specyfikacja-glosu-delty.md`](19-specyfikacja-glosu-delty.md), rozdziały o pomiarach.

## 4. Co i jak sprawdziłem

| Sprawdzenie | Wynik |
| --- | --- |
| Pełne testy całego projektu (Rust) | zielone po 0.0.34, 0.0.41, 0.0.42 i na wersji końcowej 0.0.43 (945 programów testowych, zero błędów) |
| Testy interfejsu (jednostkowe) | 329 z 329 |
| Testy w przeglądarce | 324 z 324 na 0.0.43 i 323 z 323 na 0.0.44 (pierwszy raz bez czerwonego testu); nowe scenariusze w obu motywach z kontrolą dostępności |
| Prawdziwa aplikacja (test dymny, profil tymczasowy) | 18 z 18 dla 0.0.36, 0.0.42, 0.0.43 i 0.0.44 — w tym karty „Silniki rozmowy” i „Karty graficzne na żywo” na prawdziwym rdzeniu |
| Rozmowa „na sucho” na prawdziwych silnikach | dwie wypowiedzi z rzędu rozpoznane bezbłędnie, odpowiedź wypowiedziana |
| Prawdziwy mikrofon, po cichu | potok słucha, wykrywa dźwięk, strażnik odrzuca szum |
| Zrzuty nowych ekranów | trzy style × dwa motywy, także wąskie okno |
| Pobieranie samym programem na świeżym profilu | Smart Turn i ONNX Runtime: 23 s, Supertonic 3 (16 plików, 383 MB): 101 s — wszystkie z przypiętymi sumami; po pobraniu koniec wypowiedzi przeszedł na Smart Turn, a głos Supertonic przeszedł próbę (1,8 s) |
| Uruchomiona aplikacja na profilu właściciela | wersja, silniki, próba silników i podgląd karty odczytane z działającego programu (bez klikania w okno i bez dźwięku) |

## 5. Czego nie sprawdziłem

- **Żywa mowa do mikrofonu i tłumienie echa**, gdy agentka mówi z głośnika — nie odtwarzałem dźwięku w nocy.
- **Brzmienie głosów** — ocena słuchem należy do Ciebie (próbki: `alfa-glos-proba/odsluch.html`).
- **Pobieranie modelu Qwen3-ASR (2,5 GB) i silnika `llama-server` samym programem** — sumy i adresy są przypięte
  i sprawdzone na plikach, ale tych dwóch pobrań przez program nie wykonywałem (pozostałe nowe pozycje — patrz
  rozdział 4).
- **Obowiązek codzienny z prawdziwymi danymi** — wykonanie sprawdziłem (rozdział 6), ale agentka nie dostaje
  jeszcze faktów, więc wzory są ukryte.
- **Karty AMD i Intel w podglądzie na żywo** — odczyt działa dziś tylko dla kart NVIDIA; na innym sprzęcie karta
  mówi to wprost.
- `cargo deny` (brak narzędzia lokalnie — tylko w CI).

## 6. Błędy, które sam wprowadziłem i naprawiłem

- Dwa testy zmiany głosu agentki były czerwone od 0.0.30 (nie uruchomiłem ich wtedy) — naprawione w 0.0.34.
- Biblioteka `tract` liczyła model Smart Turn błędnie (0,33 zamiast 0,95) — model działa w osobnym procesie.
- Pierwsza wypowiedź po zimnym starcie czekała do 20 s na wczytanie modelu — dodana rozgrzewka (0.0.35).
- Gotowe obowiązki (0.0.40) wypuściłem bez uruchomienia ich na modelu. Próba o 4:00 na profilu tymczasowym
  z lokalnym modelem: obowiązek wykonał się w 4 min 19 s, ale raport wymieniał nieistniejące zadania i koszty
  („5000 zł wczoraj”). Ukryłem wzory w 0.0.44; pierwsze zadanie P14 to podawanie agentce prawdziwych danych.
- W nocy zabrakło miejsca na dysku (121 GB plików pośrednich po ośmiu wersjach) — usunąłem tylko odtwarzalne pliki
  budowy testów; dane i program nietknięte. Wniosek zapisany: rzadziej podbijać wersję w serii i pilnować dysku.

## 7. Stan końcowy wersji 0.0.44

| Co | Wynik |
| --- | --- |
| Wersja | 0.0.44, zbudowana (`target/shell/debug/alfa-desktop.exe`) i uruchomiona |
| Pełne testy projektu (Rust) | 945 programów testowych, zero błędów (przebieg na 0.0.43; 0.0.44 zmienia tylko interfejs — ukrycie wzorów) |
| Testy interfejsu | 329 z 329 |
| Testy w przeglądarce | 323 z 323 |
| Test prawdziwej aplikacji | 18 z 18 |
| Linter Rust, formatowanie, kontrola zależności, kontrola stylów | bez uwag |
| Polecenia łącza | 229 |
| Aplikacja na Twoim profilu | uruchomiona o 4:35; wersja 0.0.44; „Sprawdź silniki”: rozpoznawanie działa (3,5 s, Qwen3-ASR na karcie), koniec wypowiedzi działa (0,2 s), głos działa (1,6 s); na ekranie czeka okno „Co nowego” |
| Commity | wszystkie lokalne (wypchnięcie wymaga `gh auth login`) |

## 8. Co zostaje w grupach, które ruszyłem

| Grupa | Zostaje |
| --- | --- |
| P1 Rozmowa i mowa | próba z żywą mową (Ty), kula stanu i miernik w nagłówku rozmowy, głosy Pipera z parametrami, głos Edge jako opcja (łączy się z siecią — wymaga Twojego przeglądu) |
| P2 Modele lokalne | Sklep modeli, rozkład modelu na karty, pięć stopni awaryjnego startu, poprawki szablonów, panel System, podgląd kart AMD / Intel |
| P3 Nawigacja | menu kołowe, doki, okna odczepiane, pulpit kafelkowy, ekran startowy |
| P14 Obowiązki codzienne | **najpierw** podawanie agentce prawdziwych danych (zadania, koszty, pliki) i powrót wzorów; potem łączniki poczty i kalendarza, raport z wykonania, budżet obowiązku |

## 9. Decyzje i czynności po Twojej stronie

- Posłuchać głosów i wypróbować rozmowę na żywo — tego nie mogę zrobić sam.
- `gh auth login` — bez tego ponad 130 commitów zostaje lokalnie.
- Podpisy pod ADR 0017–0023 i 0027 oraz 44 decyzje bezpieczeństwa z pakietu decyzji.
- Środowisko próbne Chatterboxa (8,4 GB) — do usunięcia, jeśli niepotrzebne.
