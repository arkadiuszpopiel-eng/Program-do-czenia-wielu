# Lokalny strumień bez postępu — #77, 10.11.2026

Kanoniczny kod8116740d22f7fa45c7b38e1540a3629fed0f5d75 wybiera dla lokalnego modelu
limity według niepustej treści po dekoderze. Pingi, Started, Usage i puste delty nie
rozpoczynają ani nie przedłużają generacji. Konfiguracja60s first-token/30s idle bez zmian.
Nie jest to przyspieszenie wag modelu lub gwarancja płynności na CPU/GPU.

Drzewo kodu jest identyczne z lokalnym df0b02c8, na którym ponownie przeszły159/0/1,
Clippy, format, lint i generatory. Rzeczywisty replay należy do lokalnego03680732,
przed mechanicznym wydzieleniem niezmienionych typów protokołu do wire.rs; funkcje
runtime run.rs i lokalny kodek pozostały identyczne. Lint wcześniej wykrył406 linii
engine.rs, a pierwsza kompilacja po przeniesieniu wymagała importu SseEvent w istniejącym
teście. Oba niepowodzenia zachowano; nie są zaliczeniami. Nie opublikowano przejściowego
commitu z naruszeniem rozmiaru w przodkach gałęzi produktu.
[Drzewa, SHA256 i źródła](artifacts/local-progress-canonical-source.json) ·
[bundle dokładnych lokalnych commitów](artifacts/local-progress-probe-sources.bundle).
Bundle zawiera również odrzucone warianty; nie włączono ich do aplikacji.

## Usterka i decyzja

Dotychczas wspólny silnik po każdej porcji bajtów ustawiał seen i nowy idle. Pierwszy ping
przestawiał60s na30s, zaś następne pingi mogły podtrzymywać żądanie bez końca. Zależnie od
chwili przyjścia bajtów występował więc przedwczesny Idle albo brak zakończenia bez treści.

Początkowo sprawdzono samą kadencję serwera: b11559 ma domyślne pingi30s, równe idle30s.
Kandydat aa02af79 ustawiał10s. RED0/1→81/0/1 iClippy0 nie wystarczyły: rzeczywisty replay
Bety/Jarvisa nie zwrócił treści w120s,121557ms do końca obserwacji, bez terminalnego zdarzenia.
Próba ciepła niewykonana; Rust0/1, kod101. Pingi rozwiązywały tylko aktywność transportu,
więc **sam wariant kadencji odrzucono**. Nie ma flagi ping10 w aktualnym kodzie aplikacji.
[Łatka](patches/local-cadence-only-rejected.patch) i surowe dane pozostają do odtworzenia.

## Zmiana kodu

- `WireCodec::content_timeouts()` ma domyślne false, zachowujące dotychczasową semantykę
  chmurowych API. Wyłącznie `LlamaCodec` wybiera true.
- Przed treścią pozostaje absolutny first-token od wysłania HTTP, również po komentarzach,
  Started, Usage lub pustych fragmentach. Po treści idle jest od ostatniego niepustego postępu.
- Treść obejmuje tekst, myślenie i wywołania narzędzi po dekoderze. Koperta JSON musi dać
  zdekodowaną treść w tym budżecie; surowe niedomknięte JSON nie jest gotową akcją.
- TTFT i after_output nie są ustawiane przez puste delty. Po treści błąd zachowuje
  after_output=true; przed treścią false umożliwia istniejącą logikę doboru trasy.
- Stop/anulowanie i upuszczenie odbiorcy nadal mają pierwszeństwo. Nie dodano retries,
  progów evals, zależności, polityk, modyfikacji promptu lub uprawnień.

Start ładowania sidecara ma odrębny, istniejący limit. HTTP first-token nie obejmuje startu
wag; ręczny pomiar widocznego tekstu obejmuje cały adapter. Nie utożsamiamy tych zegarów.

## RED i GREEN

Właściwe odtworzenie na bazie46f515bf z atrapą prawdziwego procesu/TCP: **RED0/4**, kod101.
Przypadki obejmują: komentarze bez treści, metadane/puste delty bez treści, komentarze po
treści oraz zbyt wczesne skrócenie first-token do idle przez początkowe metadane.
Po zmianie **4/4**, następnie dodatkowa kontrola Stop bez ponowienia żądania.

Pełny zestaw **lib-openai-compat + providers-api-impl + providers-local-impl**, wszystkie
features: **159 zaliczonych /0 błędów /1 ignorowana próba modelu**,30 wyników celów.
Osobno:12/0/0 biblioteka,62/0/0 API,85/0/1 lokalny dostawca. Obejmuje to dotychczasowe
testy błędów, anulowania, redakcji, narzędzi, okna, lifecycle i instalacji. Clippy wszystkich
celów/funkcji tych trzech crate'ów zakończył się kodem0 bez ostrzeżeń.

Komenda testu: `cargo +1.94.1 test --locked -p lib-openai-compat -p providers-local-impl -p providers-api-impl --all-features -j2`.
Clippy: ta sama lista pakietów, `--all-targets --all-features -j2 -- -D warnings`.
Format: `cargo +1.94 fmt --all`. Surowe komendy, kody i SHA256 są w [walidacji](artifacts/local-progress-validation.json) i scratch; wynik nowego
heada CI jest dopisywany do PR24, bez przypisywania mu CI185 poprzednika.

Zachowane błędy przygotowania: pierwszy filtr exact wybrał0 testów kadencji, więc nie był
RED ani dowodem zaliczenia. Pierwszy fixture metadanych używał usage() zawierającego również
[DONE]; dwa z czterech wyników tego wstępnego przebiegu nie odtwarzały właściwej usterki.
Po poprawieniu fixture ponowiono RED i otrzymano cztery rzeczywiste porażki. Dodatkowy test
Stop początkowo czekał na Usage, które dekoder publikuje dopiero na końcu; sygnał gotowości
zmieniono na odbiór żądania HTTP przez fixture. Te porażki pozostają osobnymi logami.

## Rzeczywisty model i ograniczenia

Osobny ręczny test odtwarza dokładną instrukcję i pytanie Bety/Jarvisa z historycznej próby.
To rzeczywisty LocalProvider/HTTP/llama/Qwen, bez Core, wymuszony CPU4/8192 kontekstu,
96tokenów/temp0, te same60/30/120s. Zimny i ciepły wariant są oddzielne. Pasywny zegar
obserwuje wyłącznie niepuste TextDelta; pomiar pingów nie zalicza widocznej odpowiedzi.
Źródła serwera i modelu mają te same przypięte SHA256 co wcześniejsze próby.

Wynik źródła03680732: **Rust0/1, kod101**,95,664s całej próby, lecz **2/2 poprawnie
zakończone odpowiedzi**,0błędów transportu. Zimna:83898ms do widocznego tekstu,89499ms
całości. Ciepła:414ms do tekstu,6061ms całości. Obie: „Jestem Betą i jestem gotowa pomóc.”
Niezaliczona asercja dotyczy **zimnego czasu widocznej odpowiedzi60s**, obejmującego start
wag. Nie wystąpił błąd FirstToken, a wariant ciepły został wykonany. Wcześniejsze
przedwczesne przypisanie kodu101 timeoutowi i pominięciu ciepłej próby zostało sprostowane
po odczycie surowych danych, również w wiadomości dla właściciela.

To nie jest kontrolowany benchmark przyspieszenia ani zaliczenie zimnego startu/GPU.
Ograniczenie pingów dowodzą osobne regresje, nie sam sukces tych dwóch odpowiedzi.
[Surowy replay](artifacts/local-progress-replay-responses.json),
[status i hash logu](artifacts/local-progress-replay-status.json),
[walidacja i hashe źródeł](artifacts/local-progress-validation.json).

Dane odrzuconej kadencji są w
[replay](artifacts/local-cadence-only-rejected-responses.json),
[statusie](artifacts/local-cadence-only-rejected-status.json) i
[walidacji](artifacts/local-cadence-only-rejected-validation.json).

Pełny poprzedni head46f515bf ma CI18510/10 iWindows1084/4; nowy kod wymaga własnego CI.
Nie wykonano odbioru RTX2060 12GB, mikrofonu, telefonu lub prawdziwego API. #58 z błędnymi
deklaracjami modelu pozostaje osobną niezaliczoną kartą. Bez merge przed świeżym niezależnym przeglądem.
