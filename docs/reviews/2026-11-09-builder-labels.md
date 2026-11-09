# Etykiety Kreatora, usterka #53

Na obrazie `52b-jarvis-kreator.png` z rzeczywistego WebView2 (rehearsal84, źródło35df604b)
widoczne były klucze `builder.group.system`, `builder.group.vision` i `color.agent.custom-2`.
Dodano brakujące tłumaczenia PL/EN i opis koloru numerem z palety. Atrapa otrzymała istniejące
już w rzeczywistym katalogu grupy; polityka i uprawnienia pozostają bez zmian.

Regresja przed naprawą: brak „Kolor3”, widoczny `color.agent.custom-3`; **1 nieudany,
1 niewykonany** po zatrzymaniu na pierwszym błędzie. Po naprawie **4/4 E2E,12,2s**:
Kreator w dwóch motywach i nazwy grup w obu językach. Pierwsza próba edycji miała błędny
katalog roboczy i uruchomiła wyłącznie stare2/2 testy; nie jest dowodem nowej regresji.

- Jednostkowe: **573/573**,103 pliki,33,72s.
- Svelte-check:0 błędów i0 ostrzeżeń; ESLint iPrettier:0.
- Build:14,68s. Budżet:JS134,8/150KB gzip, CSS13,9/30KB; pozostałe bramki zaliczone.
- Próby poprawki: **Linux/Chromium headless i atrapy**. Obraz Windows wykazał problem,
  ale poprawiony Windows/WebView2 wymaga nowego CI. Pełnego nowego zestawu905 E2E jeszcze nie zaliczono.

## Baza35df604b i ograniczenia

[CI163](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42573224604):
Chromium903/903,25,9min; jednostkowe573/573; WindowsRust3521/0/46. LinuxClippy nadal
blokują chronione importy #51, których łatka pozostaje niecommitowana do przeglądu człowieka.
[Rehearsal84](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42573224608):
Windows/WebView2 **63/0/5**,0 niestabilnych,2,8min; nowy Jarvis #50 zaliczony w3,1s.
Obieg nie włącza mikrofonu i potwierdza zachowanie uprawnień. Osobny job silników na tych samych
przypiętych wersjach zaliczony **1/0/0**,2 odfiltrowane,367,46s po dwóch wcześniejszychHTTP504.
Syntetyczny Piper→WhisperCPU:WER0, TTFB726ms, audio2243ms, STT47352ms/RTF21,11.
To nie jest odbiór mikrofonu ani sprzętu właściciela; nie usuwa historii porażek sieciowych.

## Wiążąca korekta zakresu właściciela

9.11 o06:53UTC właściciel zastąpił termin07:00UTC poleceniem usunięcia obecnych usterek,
bez nowych funkcji. Zamrożona lista: etykiety #53, zwięzłość/tożsamość Bielika #41/#48,
trwała obsada #37, chronione importy #51, diagnozaHTTP504 #52. Po rozliczeniu tej listy:
pełny raport, zapis i zakończenie prac do odwołania. Nie wracać automatycznie do ulepszeń.
Niezależny przegląd, zieloneCI i odbiór sprzętu nadal pozostają otwarte; PR pozostaje draftem.
