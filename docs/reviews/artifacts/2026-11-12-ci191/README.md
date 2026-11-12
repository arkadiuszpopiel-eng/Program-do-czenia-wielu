# CI191 / Windows115 — źródło372bfcd5

[CI191](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43150453166):10/10 zadań.
[Windows115](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43150453104):3/4, porażka rzeczywistego Core8B.

Archiwum zawiera pełne **dekodowane** logi i surowe raporty JSON; manifest podaje hash każdego pliku.
Nie są to oryginalne ZIPy logów GitHub. Oryginalny artefakt Windows11513914790894
sprawdzono SHA256082a2ed90a34129bec57c2ad08acd9a685baee31d81f67d33b37e347503795a6.

- RustLinux3631/0/28, Windows3653/0/49;1149 podsumowań libtest na system oraz osobny no_alloc.
- Vitest593/593; Chromium963/963 (321+321+321),0 pominięć,0flaky,0 brakujących/zdublowanych tytułów.
- Prawdziwy WebView2:63/0/5 pominiętych; runtime156.0.4314.58. Lista pominięć jest w logu.
- Rzeczywisty Core8B:1 próba,0 odpowiedzi, FirstToken60s,78353ms łącznie;29 przypadków NOT_RUN.
  Wejście poprawne; semantyka NOT_EVALUATED. Nie jest to wynik0/30 jakości.
- CloudCPU prefill256:18,338402tok/s,13959776816ns. Kompletny JSON przechwycony;
  kod wyjścia odłączonego dziecka niedostępny. Przy inspekcji proces był zombie, nie wymagał sygnału.
- Tokenizacja szacowanej ramki ChatML:1079tokenów. To przybliżenie szablonu, nie dokładna ramka HTTP.
  Pomiar Cloud i szacunek nie dowodzą przyczyny timeoutu Windows ani wydajności RTX206012GB.

Budżet120s wpisany w receipt benchmarku nie miał automatycznego kontrolera przerwania.
Inspekcja nastąpiła po tym oknie; wynik był już kompletny. Nie twierdzimy, że kontroler wymusił120s.
Nie wykonano ponowienia ani nie podniesiono progów. Brak odbioru telefonu, GPU, mikrofonu i API chmury.
