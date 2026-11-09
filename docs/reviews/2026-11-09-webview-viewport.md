# Viewport testu dokowania w WebView2

Karta [#40](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/40).

## Wynik rzeczywistej aplikacji na runnerze GitHub

[Rehearsal78](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42545785863)
na `74ce7f99`: **61 zaliczonych / 1 nieudany / 5 pominiętych**,4,1 minuty.
Test plakietki instancji przeszedł po poprawce flagi dla portu9222. Nadal nie przeszedł `z-night`:
nawigacja Sesji była widoczna, ale brakowało przycisku schowania w nagłówku.

Archiwum13644315614, SHA256 `55976712fd79e04ccda170823c8f61b2a14ae3021e9a41f7a795109755dc2824`:
snapshot zawiera **dialog „Sesje”**, a zrzut `42a-menu-trybu-classic-light.png` ma1028×779px.
Zgodnie z produkcyjną logiką układu szerokość poniżej1100px daje szufladę, bez przycisku dokowanego
panelu i bez zachowania „wysuń/przypnij” dla szerokiego okna. Samo otwarcie Sesji w #38 nie
zapewniło więc warunków wymaganego scenariusza. To nie dowód błędu produkcyjnego dokowania.

## Zmiana testu

Tylko fragment `z-night` sprawdzający dokowanie i wysuwanie dostaje przez CDP viewport1600×900,
ze skalą1. Helper najpierw sprawdza rzeczywiste metryki w dokumencie. W `finally` przywraca
poprzednią emulację albo usuwa override w kontekście bez viewportu, potwierdza metryki i odłącza
sesję CDP. Menu trybów przed tym fragmentem oraz dalsze testy okien leniwych działają z pierwotnymi
metrykami. Zachowano wszystkie asercje i120 sekund całego testu.

Jest to **rzeczywisty WebView2 i backend aplikacji z emulowaną szerokością tego fragmentu**,
nie próba dokowania natywnego okna większego od monitora. Nowy Windows CI pozostaje wymagany;
nie jest to odbiór na komputerze właściciela. Kod produkcyjny, polityki i progi nie zmieniły się.

## Sprawdzenie na Linux/Chromium

Trzy przypadki headless, backend na atrapach:

- przy1028px Sesje są szufladą; przy1600px można je schować, wysunąć i przypiąć; metryki wracają;
- błąd callbacku nadal przywraca viewport;
- kontekst bez zadanego viewportu usuwa emulację i wraca do swoich wcześniejszych metryk.

Końcowy wynik: **3/3**,9,5s. Pierwsza próba dwóch przypadków miała2/2; po dodaniu trzeciego
próba miała2 zaliczone i1 błąd konfiguracji: preset Desktop Chrome wymuszał `deviceScaleFactor`
niedozwolony przy `viewport:null`. Test tego wariantu jawnie usuwa tę opcję; końcowe3/3 uruchomiono
od początku. Zachowano oba logi, bez uznawania nieudanego przygotowania za sukces helpera.

`pnpm check`:0 błędów,0 ostrzeżeń. ESLint zmienionych plików: zaliczony.
Nowy pełny zestaw UI ma890 przypadków i pozostaje do wykonania; stare877/2 pozostaje zapisanym
niepowodzeniem. Te3 lokalne przypadki nie zastępują wyniku WebView2.

```text
pnpm --dir apps/desktop/ui exec playwright test e2e/docking-viewport.spec.ts
pnpm --dir apps/desktop/ui check
pnpm --dir apps/desktop/ui exec eslint e2e-app/docking-viewport.ts e2e-app/z-night.spec.ts e2e/docking-viewport.spec.ts
```

Logi: `/workspace/scratch/webview-viewport-tests.log` (pierwsze2),
`webview-viewport-tests-final.log` (2/1), `webview-viewport-tests-complete.log` (3/3),
`webview-viewport-check.log`. Archiwum Windows: `webview2-ci78-74ce.zip`.
Niezależny przegląd, pełne próby i Windows nadal otwarte; wersja0.0.85, etap J w toku.
