# Menu trybów za modalem Głosu, #47

Windows81 na974ae45e: **60 zaliczonych / 1 nieudany / 6 pominiętych**,4,8min.
Instrumentacja #42: ustawienie PIN-u i włączenie kłódki przeszły w0,607s i0,581s;
zatrzymało się kliknięcie menu trybów na118s. Sprzątanie po timeoutcie przesłaniało pierwotny błąd.

Zapisany i zweryfikowany SHA256 artefakt Windows80 (`da0b55a2…e16a0d8`) pokazuje dialog
„Panel boczny”, wybrany Głos, przycisk „Schowaj panel prawy (Ctrl+\\)” oraz formularz za nim.
`z-night` dochodzi teraz do Alt+6 i zostawia ten panel; w wąskim oknie jest on modalem.
Widoczność formularza nie oznacza, że można kliknąć jego przyciski.

## Odtworzenie i poprawka

Nowa regresja Chromium:1028×779, otwarcie Głosu, dotychczasowe przygotowanie formularza,
zwykły klik menu z5s. **1 nieudany / 1 zaliczony** (szeroki wariant),14s. Log wprost pokazuje
`<button class="scrim …"> intercepts pointer events`; to potwierdza blokującą warstwę modalu.

Wspólny helper `prepareComposer` wraca z Ustawień, zamyka widoczny dialog przyciskiem panelu,
czeka na jego zniknięcie i zwraca formularz. Używa go rzeczywisty test kłódki i regresja Chromium.
Szeroki niemodalny Głos pozostaje otwarty. Po zmianie **2/2 zaliczone**,7,9s;
ten sam klik5s, bez `force`, syntetycznego `dispatchEvent` ani zmiany polityki.
Native120s,30s widoczności formularza i wszystkie asercje PIN-u/kłódki pozostają dotychczasowe.
Svelte/TypeScript0/0, ESLint i format zielone. Nowy Windows jest nadal wymagany.

Logi: `/workspace/scratch/composer-ready-{red,green,check}.log`.
Produkcja UI pozostaje taka sama jak w pełnym899/899 naaf5a2bc5; dwa nowe testy dotyczą
prewarunku harnessu. Nie nazwano tego pełnym901 lokalnie. To Chromium/fake, osobno od WebView2.

ZIP Windows81 z FileService nie przeszedł kontroli: otrzymano46415206 zamiast46641406 bajtów,
SHA256 niezgodne, brak poprawnego końca ZIP. Plik zachowano jako `.zip.partial` i nie użyto jako
dowodu. Podstawą są zweryfikowany snapshot80, log81 z GitHub i odtwarzalna regresja Chromium.
Metadane artefaktu81:13647754798, oczekiwany SHA256
`1956cb411b1e90d5b595dae2ca63661a11e32ccf290e1953b2bbc34a2ad5c0f2`.
