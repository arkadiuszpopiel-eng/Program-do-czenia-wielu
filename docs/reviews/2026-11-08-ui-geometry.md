# Menu trybów i ustawienia w małym oknie

Karta [#32](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/32).
Baza `988d13f3`, źródła UI `1abe0861`; Linux Cloud, Chromium Playwrighta, dane syntetyczne.
Wyniki są odrębne od Windows/WebView2 i sprzętu właściciela.

## Odtworzenie i poprawka

Przy wysokości500px menu pięciu trybów zaczynało się396,03px nad oknem. W oknie360px miało
lewą krawędź193,47px poza ekranem. Popover wybiera większe dostępne miejsce nad/pod przyciskiem,
ogranicza wysokość i przewija treść; poziome przesunięcie mieści panel w oknie. Obliczenia
przeliczają rzeczywiste prostokąty okna przez CSS zoom. Zmiana rozmiaru, skali i doładowanie Lazy
odświeżają geometrię; obserwatory są odłączane przy zamknięciu.

Odłożenie8px przy przewijaniu zapewnia pełną widoczność wiersza również przy ułamkowych
współrzędnych. Po pierwszej poprawce13/18 testów przechodziło, pięć wciąż widziało99,3–99,9%
wiersza. Wymaganie100% pozostało; poprawiono `scroll-padding`, nie detektor.

Przyciski Kont API w360px z większą skalą mieściły się tylko w około68,4%. Włączono istniejące
zawijanie `Button.wrap` i ograniczono kontener akcji. Problemów Wtyczek dotyczyły prostokąty
wielowierszowych spanów: zrzut pokazuje czytelny tekst w jednym akapicie, a test wykrywa ich
przecinające się ramki. Zastosowano standardową listę `wk-list`, rozdzielając status, nazwę
i szczegóły na wiersze. Detektor i progi pozostały niezmienione.

| Przed | Po |
| --- | --- |
| [Menu360px](artifacts/menu-narrow-before.png) | [Menu360px](artifacts/menu-narrow-after.png) |
| [Problemy Wtyczek](artifacts/plugins-before.png) | [Problemy Wtyczek](artifacts/plugins-after.png) |

## Sprawdzenie

- Nowe E2E: **23/23** — cztery style, wysokości500/650/800px, skala zwiększana skrótem i potwierdzana
  stylem obliczonym, szerokość360px, radio zakresu, każdy z pięciu wierszy i zmiana rozmiaru już
  otwartego menu; przyciski kont i oba motywy Junior. Wybór Ręcznie zachowuje zwykłą ścieżkę komendy.
- Vitest bez konkurujących kompilacji/modelu: **573/573**,103 pliki,28,40s.
- Svelte/TypeScript: **0 błędów,0 ostrzeżeń**; lint, Svelte5, CSS i rozmiary zaliczone.
- Bundle: JS134,8KB/150KB, CSS13,9KB/30KB, Szybkie pytanie39,2KB/40KB (gzip) — bez zmian limitów.
- IPC397typów/285komend i100stron dokumentacji sprawdzone. Pełny E2E nowego UI do wykonania;
  starsze856/856 na9b8b7416 oraz porażki CI149 są osobnymi wynikami.

Próby czerwone: menu1 nieudany/11 nieuruchomionych; po naprawie nawigacji testowej ustawień
4 zaliczone/1 nieudany/1 nieuruchomiony (Wtyczki); powiększone Konta1 nieudany/3 nieuruchomione;
po pierwszej poprawce13 zaliczonych/5 nieudanych; wąskie menu1 nieudany/3 nieuruchomione.
Pierwsza próba ustawień utknęła wcześniej w setupie (wyścig rozpoznania zwiniętej nawigacji);
poprawiono oczekiwanie na przycisk sekcji. Log tej próby również zachowano. Diagnostyka kończyła
czerwone przebiegi po pierwszej porażce; końcowy zestaw23 wykonano w całości, bez wymuszonych kliknięć.

Logi: `/workspace/scratch/ui-{mode-viewport-red,settings-geometry-setup-failure,settings-geometry-plugins-red,
accounts-geometry-red,geometry-first-fix,mode-narrow-red,geometry-bounded,geometry-unit-quiet,
geometry-types,geometry-lint}.log`. Przypadki geometrii nie są pomiarem szybkości; uruchamiano je
pojedynczo podczas Rust. Vitest czasowy wykonano po zakończeniu Rust.
Przegląd niezależny i pełne CI otwarte; wersja0.0.85, etap J w toku.
