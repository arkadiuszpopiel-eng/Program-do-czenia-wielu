# Wyszukiwanie bezAPI — dowody pierwszego przyrostu

- browser-search-red:0/1, brak narzędzia w prawdziwym zestawie, kod101.
- browser-search-green: błąd kompilacjiE0603, zły import aliasuStep; nie semantycznyRED.
- browser-search-contract-green:18/0/0 dla kontraktu/impl/fake, wcześniejszy etap.
- browser-search-final: błąd kompilacjiE0599 w teście, nieistniejąca nazwa reguły; nie semantycznyRED.
- destination-red:0/1, stara strona błędnie oznaczana jako wyszukanie; kod źródłowy tegoRED w archiwum.
- browser-search-complete:55/0/3, kontrakt/impl/fake iapp-agents. Regresja adresu już zaliczona.
- catalog-red:0/1 (spis nie zawiera nowego browser_search), potem oficjalny generator1/1,
  następnie zwykły test bez trybu aktualizacji1/1. Spis zmieniony dokładnie o1nazwę.
- Clippyworkspace/all-targets0bezostrzeżeń; deny4OK (bazowe ostrzeżenia zachowane).
- fmt/graf1812krawędzi/generatorTS/docs104/format/lint:0; oba lockfile sprawdzone metadata--locked.
  Starsze wersje/źródła/checksums paczek nie zmieniły się, doszła krawędź doform_urlencoded1.2.2.
- Katalog funkcji612/612 ma ślad testu; nie jest liczbą zaliczonych prób runtime.

Trzy pominięcia: real_git_and_cargo_tests, measure_on_local_model, real_machine_description.
Brak wykonania prawdziwej wyszukiwarki/WindowsBrowserPort/telefonu/GPU/API. Nie ma osadzonegoWebView2.
Testy nie wysyłają zapytań doInternetu; hosty i strony są syntetyczne. RaportyTauri metadata
opisują rozwiązanie zależności naLinux, nie kompilację aplikacjiWindows.

Testowany kod bazuje na5f265ba3 z workingdiff. Cherry-pick poprawia wyłącznie lockfileTauri/dokumenty;
końcowe bajty kodu i generatora spisu są związane przez browser-search-final-source.json.
Archiwum ma pełne logi, receipty i hashe; błędne etapy zachowane wraz z końcowymi wynikami.
