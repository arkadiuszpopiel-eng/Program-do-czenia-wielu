# Pobieranie po chwilowym HTTP504, #52

HTTP504 pochodził z pobierania zewnętrznych archiwów. Dotychczasowy klient już ponawiał5xx/429,
ale wykonywał kolejne próby bez przerw. Nowa regresja z serwerem odzyskującym dostępność
po200ms odtworzyła wyczerpanie prób przed odzyskaniem dostępności: **3 zaliczone/1 nieudany,0,09s**.

Dodano anulowalne przerwy **250/500/1000ms** dla5xx/429. Budżet nadal wynosi3 wznowienia
(cztery żądania), timeouty15/30s i przypięte adresy/hashe są takie same. Zerwany strumień iRange
zachowują dotychczasową ścieżkę. Trwałe504 kończy się jawnym błędem, pozostawiając plik częściowy;
błądHTTP nie staje się treścią pobieranego pliku. Łączne nowe czekanie wynosi najwyżej1,75s.

## Dowody lokalne

- Nowe regresje **4/4,1,83s**: czasowa awaria504, poprawneRange iSHA256; niezmienione cztery
  próby przy trwałym504; anulowanie oczekiwania429;404 bez ponowień.
- Cały `app-models` z domyślnymi cechami: **82 zaliczone/0 nieudanych/1 pominięty**,16 wyników celów.
- Clippy pakietu,wszystkie cele,`-D warnings`: **0 ostrzeżeń,1m38s**.
- [Komendy, hashe źródeł i logów](artifacts/http-outage-validation.json).

To kontrolowany serwer naLinuxie. Dostępności zewnętrznegoCDN nie można zagwarantować kodem klienta.
Nowy pełnyCI i przebieg silników Windows po publikacji pozostają wymagane.

## Historia pobierania Windows

83: dwie porażki504 (288,66s i248,19s).84: zaliczenie1/0 w367,46s.
85:0/1,2 odfiltrowane,207,90s;504 sześciu archiwów.86 na9d2b35fa, **przed nową poprawką**:
1/0,2 odfiltrowane,280,50s; obaLLM i syntetyczny głos,WER0,STT37198ms/RTF16,17.
To potwierdza okresowość błędu zewnętrznego; nie dowodzi wpływu nowej przerwy na ten konkretny przebieg.
CloudHEAD tego samego archiwum llamaCPU zwrócił200 (19 501 338B), bez deklaracji pełnego pobrania.
Historia niezaliczonych prób pozostaje zachowana; progów, polityk, katalogu i wersji nie zmieniono.

## Pełna baza po #37 i zatwierdzonym #51

[CI165](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42586480252)
na9d2b35fa: pełnyLinux **3500/0/25**,1124 wyniki celów,`--all-features` i zielony pełnyClippy.
Chromium **905/905,23,6min**,jednostkowe573/573; typy0/0,lint/build/budżet/Storybook.
Tauri Windows (w tym pełny`cargo metadata --locked`),cargo-deny i dokumentacja zielone.
PełnyRustWindows jeszcze trwa. Rehearsal86:WebView2 **63/0/5**,Jarvis2,7s,bez uruchamiania mikrofonu.
Syntetyczny głos nie stanowi odbioru mikrofonu, urządzeń właściciela ani czasu rzeczywistego.

Zwięzłość i tożsamość Bielika pozostają osobną otwartą usterką; kandydatów promptu nie wprowadzono
przed pełnym sprawdzeniem. Nadal obowiązuje zakres samych napraw, potem raport i STOP do odwołania.
