# Jarvis: formy imienia i długość odpowiedzi

Karta [#31](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/31).
Próba na Linux Cloud CPU, llama.cpp b11559, Bielik 1.5B Q8_0 o SHA256 zapisanym w raporcie nocnym.
Rzeczywisty model; headless AppCore, atrapy audio i portów Windows, bez mikrofonu i narzędzi.

## Próba bazowa

Źródła `e8baf1da`; [surowe odpowiedzi](artifacts/jarvis-bielik-baseline.json).
40 świeżych rozmów: 4 persony × 2 profile × 5 powtórzeń, temperatura 0, limit 96 tokenów,
120 s na turę, 4 wątki. Bez równoległych kompilacji ani testów czasowych UI.

**40/40 ukończonych, 0 błędów transportu, 0 obciętych odpowiedzi, autor zgodny w 40/40; 215,88 s.**
To wynik integracji, nie zaliczenie jakości. W Standardzie Beta 5/5 podaje „Betta”, Gama 5/5 „Gamma”.
W Jarvisie Beta 5/5 poprawnie używa „Betą”; Gama 1/5 „Gamą”, 4/5 „Gamma”. Wiele odpowiedzi
ma kilka zdań i wylicza cechy mimo prośby o jedno krótkie zdanie. Alfa i Delta zachowują imiona.
Pole `mentions_own_name` sprawdza podciąg, więc poprawne „Betą” i „Gamą” daje fałsz.
Ocena form imienia powyżej jest odczytem autorskim wszystkich odpowiedzi.

Mediany czasu: Standard 5472 ms, Jarvis 2984,5 ms. Stała kolejność i zimny start wykluczają wniosek
o przewadze szybkości profilu. Tokeny wejścia raportowane przez dostawcę mogą nie obejmować cache.

## Poprawka i regresja

Jarvis przekazuje bieżące imię i narzędnik persony z katalogu, np. „Gama” / „Jestem Gamą”.
Przebiegi i delegacje korzystają bezpośrednio z persony specyfikacji, w tym własnej.
Instrukcja wymaga długości wskazanej przez właściciela i opisywania cech oraz ról na prośbę.
Tekst odpowiedzi nie jest podmieniany. Standard, role, uprawnienia, limity i format Krytyczki
pozostają dotychczasowe. Nie zmieniono progów ani parametrów próby.

Regresja rdzenia przed poprawką: **1 nieudany**, brak jawnych form imienia Alfy.
Po poprawce na `1ecbcfba`: **43 testy app-chat + 5 Jarvisa + 1 głosu zaliczonych**, 0 nieudanych.
Pokrywa cztery persony, ich nadpisania, zapis/restart, przebieg z narzędziami, delegację,
zadanie w tle i syntetyczne audio. Test jednostkowy używa również własnych form „Luna” / „Luną”.

## Powtórka po poprawce

Ten sam protokół na `1ecbcfba`: **40/40 ukończonych, 0 błędów transportu, 0 obcięć,
autor zgodny 40/40; 196,93 s**. [Surowe odpowiedzi](artifacts/jarvis-bielik-refined.json).
Standard daje dokładnie te same teksty co w bazie. W Jarvisie każda persona 5/5 odpowiada:
„Jestem Alfą/Betą/Gamą/Deltą. Jestem gotowa pomóc.” Poprawne formy imienia **20/20**,
bez wyliczania cech i ról. To poprawa w wąskiej próbie przedstawiania się.

Warunek jednego zdania **0/20** w Jarvisie: model stale daje dwa zdania. Wskazówka długości
nie wystarczyła; pełna jakość polecenia pozostaje **niezaliczona**. Nie poprawiano tekstu ani
nie osłabiono kryterium. W Standardzie błędne „Betta” i „Gamma” pozostają do osobnej diagnozy.
Mediany Standard 6034 ms / Jarvis 1115 ms są opisowe, nie porównaniem wydajności profili.

Pierwszą powtórkę przerwał restart Cloud po 5 odpowiedziach Standardowych —
[częściowy raport](artifacts/jarvis-bielik-refined-interrupted.json). Nie była zaliczona;
następna próba powyżej zaczęła wszystkie 40 tur od nowa i zakończyła się kodem 0.
Clippy app-chat/app-core (lib i zmienione cele testów, `-D warnings`) zaliczone przed próbą.
Serwer modelu został zatrzymany; nie nakładano kompilacji i testów czasowych UI.

Dalej: bardziej ogólna próba poleceń i długości, własna persona w integracji, pozostałe regresje
Linux oraz [UI #32](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/32).
Przegląd w świeżej sesji i próba sprzętowa pozostają otwarte. Wersja 0.0.85, etap J w toku.
