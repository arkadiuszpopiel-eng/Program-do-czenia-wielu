# Wyszukiwanie w przeglądarce bez klucza API — pierwszy przyrost

Właściciel wybrał tę ścieżkę jako pierwszą w #78. [Zapis decyzji](2026-11-12-owner-decisions.md).
Nowe narzędzie `browser_search` jest w rzeczywistym zestawie narzędzi agentek. Wymaga
jawnego silnika DuckDuckGo/Google/Bing i frazy; nie wybiera domyślnego endpointu.

## Zachowanie

Fraza staje się jedną wartością parametru q. Kodowanie obejmujeUTF8 i znaki składniURL;
limit4096bajtów istniejącegoURL obowiązuje po kodowaniu, bez obcinania tekstu lub nowego progu.
Brak kluczaAPI, cookies użytkownika i nowego konta. Nowa krawędź form_urlencoded1.2.2
używa już przypiętej paczki; oba lockfile zachowują wcześniejsze wersje/źródła/checksums.

Wywołanie przechodzi dokładnie przez istniejącą ścieżkę browser_open/Broker/BrowserPort.
Kontrola hosta jest przy każdej próbie, także po wcześniejszej zgodzie; CDN i inne hosty
nie dostają automatycznego zezwolenia. Wynik pozostaje taintWeb. Anulowanie przed
wywołaniem i odmowa Brokera dają0uruchomień przeglądarki/0żądań w testach atrapy.

Sukces potwierdza tylko nawigację i końcowyHTTPS/host silnika, **nie odczyt lub ranking**.
Pole search_stage=navigation_only kieruje do browser_read, potem oceny i cytowania źródeł.
Stara strona, about:blank, inny host, podobna domena lubHTTP nie dostają tego oznaczenia;
zwracany jest błąd z rzeczywistym stanem strony. CAPTCHA, blokada, puste wyniki lub
problemyCDN muszą być rozpoznane i zgłoszone po odczycie, nie zamienione w fikcyjny sukces.

To narzędzie agentki nad istniejącym BrowserPort. Backend nadal uruchamia oddzielny
Edge/Chrome zgodnie z obecną konfiguracją (domyślnie headless); **nie dodano panelu,
osadzonegoWebView2, APIsearch ani gwarancji jakości rankingu**. Przyszły osadzony backend
musi przejść osobny odbiór izolacji. Nie zmieniono Brokera, deny-list, polityk lub evals.

## Dowody

| Kontrola lokalnaLinux | Wynik |
|---|---|
| Nowe narzędzie obecne w zestawie | RED0/1, potem integracja zaliczona |
| Stara/innegohosta strona błędnie opisana jako wyszukanie | RED0/1 → kontrola zaliczona |
| Kontrakt/impl/fake + app-agents | **55passed/0failed/3ignored** |
| Faktyczny katalog narzędziCore | RED0/1 → generator → **1/1** bez trybu aktualizacji |
| PełnyClippyworkspace/all-targets | **0, bez ostrzeżeń** |
| cargo-deny | **4OK**, zachowane bazowe ostrzeżenia |
| fmt, lint, graf zależności, generatory, format | **0** |
| metadata--locked powłoki | **0**, obie zależności w osobnym lockfileTauri |

Trzy pominięcia: real_git_and_cargo_tests, measure_on_local_model, real_machine_description.
Nie wykonano rzeczywistego silnikaWWW/WindowsBrowserPort, GPU, mikrofonu lub telefonu.
Testy używają syntetycznych stron i atrapy przeglądarki; Broker korzysta z rzeczywistego silnika i portów testowych.
Nie wysyłano prywatnych zapytań ani nie czytano sekretów. Błędy kompilacji samych testów
E0603 iE0599 zachowane; nie są behawioralnymRED. [Pełne logi, źródła i hashe](artifacts/2026-11-12-browser-search/README.md).

Generator katalogu dodaje dokładnie browser_search; bramka612/612 ma ślad testu,
nie udaje612zaliczonych testów lub odbioru aplikacji. Nie modyfikowano listy ręcznie.

## Integracja i otwarte sprawy

Przygotowano poprawkę osobnego lockfile powłoki pominiętego przy lib-client-wire.
CI192 na5f265ba3 ma czerwony jobTauri przed kompilacją; fixff68914f odtwarza metadataRED101→0.
W tej gałęzi jest ta sama poprawka oraz dodatkowa krawędź nowego narzędzia. [Dowody lockfile](artifacts/2026-11-12-shell-lock/README.md).
Pozostałe zadaniaCI192 i Windows116 zachowano w toku, bez anulowania następnym pushem.
Nie przenosimy CI191/Windows115 lub CI192 na nowy kod wyszukiwania; wymaga własnegoCI
oraz świeżego niezależnego przeglądu przed merge. #78 i #69 nadal otwarte.
#58 nadal ma nieodebraną jakość: na372bfcd5 Core8B FirstToken60s,0tekstu,29NOT_RUN.

Następne prace: próba publicznego zapytania przez prawdziwyBrowserPort, błędy/CAPTCHA,
potwierdzone źródła, izolowanyWebView2 i panel mobilny; osobno transportE2E/parowanie/dowody zgód.
