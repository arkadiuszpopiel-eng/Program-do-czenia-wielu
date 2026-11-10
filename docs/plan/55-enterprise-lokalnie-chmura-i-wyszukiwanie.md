# 55 — niezawodność lokalnie, w chmurze i wyszukiwanie WWW

**Odbiór kodu10.11:**77a85caa ma [CI184](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42884779747)
**10/10**, Linux3595/0/26, Windows3617/0/47, Chromium945, Vitest573;
[Windows107](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42884779841) **4/4**, WebView2 63/0/5.
Zamknięto47 zweryfikowanych kart. GłównyQwen8B:50/50 tożsamości i50/50 świeżych pytań
bez fikcyjnych działań, w tym Luna po restarcie. #58 obejmuje odrębny brak faktów wcześniejszej
historii i pozostaje w pracy; #77 historycznyIdleCPU nadal otwarty. Telefon/API/GPU/mikrofon
oraz niezależny przegląd nieodebrane. [Raport i dane](../reviews/2026-11-10-defect-closure.md).
Starsze wpisy poniżej są historią, nie aktualnym stanem odbioru.

**Domykanie usterek10.11:** #64 deadline68/0/0, #73 jawna odmowa135/0/0, #75 pomiar
zakończenia procesu59/0/1 i20/20 anulowań ×3 mosty-atrapy przy2s; Clippy0. Baza574b3391
ma CI182 **9/10** (Linux3580/1/26, #75), Windows105 **4/4**, WebView2 63/0/5.
[Raport i ograniczenia](../reviews/2026-11-10-defect-closure.md). Nowe poprawki wymagają własnegoCI;
Core Qwen8B, sprzęt i przegląd pozostają otwarte. Bez merge i zmiany polityk/progów.

**Nowe zlecenie właściciela po zamknięciu nocnego okna:** główne modele lokalne8–12B
naRTX2060 12GB z zapasem kontekstu. Qwen3-8B Q4_K_M domyślny, Bielik11B Q4_K_M jako
alternatywa tekstowa;1.5B tylko pomocniczy. Dostępna Luna/Haiku pomaga po problemie lokalnym;
trudne zadania i zgody zachowują hierarchię. [Zmiany, testy i pamięć](../reviews/2026-11-10-model-roles.md).
To nowe polecenie w określonym zakresie; poprzedni harmonogram nie jest wznawiany.
Poniższe terminy i wyniki są historią zamkniętego okna, nie limitem nowego zlecenia.

**Kandydat #63 — oczekiwanie na magazyn pobrania:** Stop i istniejący deadline kończą
oczekiwanie na begin/write. Rozpoczęte I/O może jeszcze wracać w tle; sprzątanie następuje po
powrocie pracownika. Rozpoczęty commit zachowuje rzeczywisty wynik. RED2/4 → GREEN62/0/0;
Clippy i format kod0. [Raport #63](../reviews/2026-11-10-download-wait.md).

**Opublikowany #62 w cea4444a:** pełne **CI175 10/10 i Windows97 4/4 zielone**;
Linux3544/0/26, Windows3570/0/47, Chromium915/915 i WebView2 63/0/5.
[Dowody](../reviews/artifacts/ci175-disk-baseline.json). Nowy #63 wymaga własnego CI/przeglądu.

**#58 — próba faktów narzędzi odrzucona:** prawdziwy Core + Bielik 1.5B, po 50 odpowiedzi
obu wersji i 50/50 fikcyjnych potwierdzeń w każdej, mimo poprawnie przekazanych liczników 0.
Kod aplikacji przywrócony do 9319; [raport, łatka i 100 surowych odpowiedzi](../reviews/2026-11-10-branch-tool-facts.md).
#41/#48 nadal otwarte; nie uznajemy zielonego transportu za zaliczenie jakości.

**Kandydat[#57](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/57):**
propozycja narzędzia wymagająca potwierdzenia zachowuje osobny stan w rozmowie i Replay,
bez oznaczania jako wykonana. [Wyniki lokalne i ograniczenia](../reviews/2026-11-10-tool-confirmation.md).
Baza9319 ma zieloneCI172 iWindows94:Linux3523/0/26,Windows3549/0/47,Chromium915/915,
WebView2 63/0/5. Niezależny przegląd i odbiór sprzętu pozostają otwarte.


**Nowe okno właściciela:10.11.2026,Europe/Warsaw.** Rozwój i poprawki do05:30
(04:30UTC), potem wyłącznie końcowe testy i raport, zakończenie06:00 (05:00UTC).
Późniejsza korekta właściciela ma pierwszeństwo. Długie testy uruchamiać odpowiednio wcześnie;
nie zaczynać nowych prób po terminie i jawnie zgłosić niewykonane lub nieukończone kontrole.

**Baza23300f82 ma zieloneCI170 iWindows92.** Linux3514/0/26,Windows3540/0/47;
E2E302+302+301=905,bez porażek,w8,1–9,7min na część. WebView2 63/0/5. Nie jest to
odbiór GPU12GB,prawdziwegoAPI lub mikrofonu. Kolejna naprawa[#56](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/56)
dotyczy sekretów w URL i limitów wyników wyszukiwania; [raport](../reviews/2026-11-10-search-results.md).


## Zakres zlecony 9.11.2026

Właściciel poleceniem `/goal` wznowił naprawy i ulepszenia w kierunku jakości Enterprise:
współpraca modeli lokalnych i chmurowych, samodzielne działanie każdego wariantu, skuteczne
narzędzia i dopracowane wyszukiwanie internetowe. To zastępuje wcześniejsze zamrożenie nowych
funkcji i STOP po raporcie; stary termin nocnego okna nie obowiązuje. Sprzęt docelowy: RTX2060
12GB. Nie zmienia to progów evals, polityk ani wymagania przeglądu chronionych modułów.

„Enterprise” jest celem i zakresem odbioru, nie etykietą już osiągniętej jakości. Każdy przyrost
ma odtwarzalny scenariusz, wynik przed/po, jawne ograniczenia oraz osobny przegląd przed scaleniem.
Bez nowych mostów CLI, sekretów z plików lub samoczynnego wznawiania nocnego harmonogramu.

## Stan wyjściowy

- Produkcja050f86c7 ma zielone pełneCI168 i Windows90; e3f70a8b dodaje diagnostykę modeli.
  Status nowegoCI169/Windows91 trzeba przypisywać właśnie temu commitowi, nie kolejnym zmianom.
- #41/#48 nadal otwarte. Bielik1.5B:50/50 fikcyjnych potwierdzeń wykonania przy braku narzędzi.
  Qwen3-8BQ4 ma30/30 poprawnych treści w sondzie, ale pełnyCoreCPU niezaliczony na limicie czasu.
  To nie jest powód do zwiększania limitów lub automatycznej zmiany modelu domyślnego.
- Narzędzia sieciowe mają odczyt HTTPS i pobieranie, ale `net_search` jest portem bez produkcyjnego
  dostawcy (`tools-net/SPEC.md`, `app-agents/src/sysnet.rs`: `search: None`). Atrapa wyszukiwarki
  i możliwość pobrania URL nie są wdrożoną wyszukiwarką WWW.
- Inspekcja Routera ujawniła #54: jeśli adapter ignoruje token, Stop może pozostawić trasę
  zawieszoną. Naprawa w osobnym worktree `fix/enterprise-router-cancel` jest pierwszym przyrostem.

## Kolejność wykonania i dowody

| Priorytet | Zakres | Warunek odbioru i dowód |
| --- | --- | --- |
| 1 | Anulowanie #54 | Zawieszony adapter lokalny/API: przed zdarzeniem, po Started, częściowym tekście i rozpoczęciu narzędzia; jeden Cancelled, brak zapasu, zwolniony strumień i próba half-open; RED→GREEN i pełne testy Routera |
| 2 | Współpraca tras i narzędzi | Lokalny→API, API→lokalny oraz każda trasa osobno. Zachowane całe żądanie, identyfikatory narzędzi i wyniki; błąd po rozpoczęciu wywołania nie odtwarza go na innym modelu |
| 3 | Fakty o wykonaniu | Odpowiedź rozróżnia zaplanowanie, próbę, zgodę, odmowę i wynik potwierdzony. Dowód pochodzi z rzeczywistej historii narzędzi; nie z deklaracji modelu. Brak poświadczeń wykonania po samym tekście „gotowe” |
| 4 | Adapter wyszukiwania | Wybrany dostawca przez SearchPort, jawny stan konfiguracji, HTTPS/Broker/taint jak w istniejącym kontrakcie; kontrolowane błędy, anulowanie, ograniczone wyniki i sprawdzony schemat. Osobna karta i worktree tools-net |
| 5 | Jakość wyników WWW | Powtarzalny zestaw zapytań PL/EN: fakt, aktualność, dokumentacja techniczna, zapytanie wieloznaczne, brak wyników. Zachowane surowe wyniki i źródła; ręczna ocena trafności, aktualności i pokrycia przed deklaracją jakości |
| 6 | Odbiór systemowy | Rzeczywista aplikacja: lokalnie/offline, API i hybryda; restart, odcięcie sieci, niedostępny model, 429/5xx, odrzucone narzędzie, anulowanie, limit zadania. Osobno GPU12GB, WebView2, mikrofon i chmura z uprawnionym kontem |

Każdy wiersz pozostaje otwarty, dopóki nie ma właściwego dowodu. Atrapy pozwalają deterministycznie
odtworzyć awarie, lecz nie zastępują dostawcy zewnętrznego. Próby modelu CPU i czasowe UI wykonuje
się kolejno. Nie ponawiać niezaliczonego przypadku tylko po to, by pokazać pojedyncze zaliczenie.

## Odbiór wyszukiwania i badań

1. **Zapytanie i wyniki:** poprawne PL/EN i znaki Unicode, parametry przekazane dostawcy,
   limit wyników, wykrycie błędu dostawcy zamiast fikcyjnej pustej listy, deduplikacja adresów.
2. **Źródło:** tytuł, oryginalny i końcowy URL, data odczytu oraz data publikacji, gdy znana;
   brak daty jest jawny. Priorytet źródeł pierwotnych; ważne twierdzenie porównane ze źródłami.
3. **Odpowiedź:** cytowanie prowadzi do faktycznie zwróconego/odczytanego źródła. Snippet nie
   dowodzi całej treści artykułu. Rozbieżności i brak potwierdzenia są wskazane w odpowiedzi.
4. **Odporność:** 429/5xx, niepoprawne JSON/HTML, przekierowanie, wolny odczyt, za duża odpowiedź,
   anulowanie i niedostępny dostawca nie zawieszają zadania i dają użyteczny wynik błędu.
5. **Ograniczenia dostępu:** wyniki z sieci pozostają niezaufane; polecenia w artykule nie sterują
   agentką. Zachowane istniejące wymagania Brokera, DNS/SSRF, prywatności i redakcji sekretów.
6. **Porównanie:** wspólny zestaw pytań i jawna metodologia; nie deklarować „światowego poziomu”
   na podstawie samej dostępności endpointu. Nowe testy jakości nie zmieniają zamrożonych evals.

Wybór i konfiguracja dostawcy są częścią implementacji. Nie podpinamy przypadkowej publicznej
instancji ani logowania właściciela jako domyślnej usługi. Test żywy wymaga skonfigurowanego
uprawnionego dostawcy; wpis w katalogu narzędzi nie jest dowodem działania wyszukiwarki.

## Dodatkowa wykryta usterka CI

CI169 ma905/905 zaliczonych scenariuszyChromium, ale krok przekroczył30 minut i zadanie jest
nieudane. [#55 — trzy części302+302+301](../reviews/2026-11-09-ci-ui-shards.md) zachowują cały
zbiór i istniejące progi. Naprawa wymaga rzeczywistego nowegoCI; nie deklarować jej zaliczenia
na podstawie samej listy testów. Pozostałe zadaniaCI169 i wszystkieWindows91 są zielone.

## Stan i następny przyrost

Pierwsza karta: [#54](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/54).
Raport: [Router i przypadki awarii](../reviews/2026-11-09-enterprise-router.md).
Pozostałe wiersze wymagają osobnych przyrostów i testów; ten plan nie oznacza ich wykonania.
Nie scalać bez świeżego niezależnego przeglądu i zielonego CI. Odbiór sprzętowy pozostaje oddzielny.


## Kodowanie 10.11

Nowe polecenie właściciela 10.11 po zamknięciu prac nocnych: asystentki mają tworzyć,
kompilować, uruchamiać, testować i poprawiać programy z wykorzystaniem narzędzi komputera.
Stare harmonogramy pozostają wyłączone. Ten plan nie oznacza zakończonego odbioru Enterprise.

### Zakres bieżącego przyrostu

1. #70: deterministyczna próba prywatności terminala. Łatka zwykłego testu warstwy app-core
   została opublikowana w bf9ffe05; wcześniejsza kwalifikacja jako chronionego core-* była
   zbyt szeroka. Bez zmiany polityk lub tagów. [Raport](../reviews/2026-11-10-terminal-privacy.md).
2. #71: zachowanie końcowych błędów kompilatora w ograniczonym wyniku narzędzia. Początek,
   jawne pominięcie środka i końcówka mieszczą się w istniejącym limicie; bez zwiększania progów.
3. Test integracyjny runtime: skryptowane decyzje modelu i rzeczywisty Rust w prywatnym katalogu:
   błąd kompilacji → zapis poprawki → kompilacja testu → test → uruchomienie → przegląd tylko do odczytu.
4. Osobne małe próby rzeczywistego Qwen8B na CPU: błąd typu i błąd logiczny. Surowe odpowiedzi,
   argumenty narzędzi, wyjścia procesów i czasy zachowane. To sondy poza Core/Brokerem.
5. #72: kod0 bez rozpoznanego, wykonanego testu nie potwierdza zaliczenia. Pusty wynik,
   zero testów po filtrze i same pominięcia mają jawny stan niepotwierdzony; kod procesu
   jest również w tekście dla modelu. RED6/3 → GREEN14/0/0; wspólne207/0/0 i Clippy0.

### Uprawnienia i możliwości

Rola Koderki już obejmuje worktree i powłokę; `dev_git` i `dev_test` należą także do grupy
`shell`. Kompilację i uruchomienie realizują istniejące narzędzia, a nie tekst odpowiedzi.
`shell_terminal` przedstawia propozycję dla właściciela; `shell_run` wykonuje polecenie.
Prywatny terminal logowania CLI pozostaje przeznaczony dla właściciela; asystentki korzystają
z własnych narzędzi wykonawczych, bez odczytu tokenów i strumienia logowania.

Właściciel wybiera istniejący Pełny/Max dostęp dla lokalnych/chmurowych modeli. Zmiana modelu
lub persony nie nadaje dodatkowych praw. Instalacja pakietów podlega istniejącemu Brokerowi;
Windowsowe podniesienie praw nadal wymaga rzeczywistego tokenu UAC. Cloud Linux nie potwierdza
instalacji sterownika, rebootu, pracy ConPTY ani obsługi docelowego GPU.

Główne modele lokalne pozostają 8–12B z rezerwą kontekstu na RTX2060 12GB. Bielik1.5B jest
pomocniczy. Skonfigurowany lekki model chmurowy może pomóc po problemie lokalnym; trudne
zadania zachowują dotychczasową hierarchię. Nie ma potwierdzonego konta/API tej sesji, więc
nie deklarujemy wykonanej próby Luna/Haiku ani instalacji nowego dostawcy.

### Kolejność dalszego odbioru

1. Pełne CI aktualnego heada i niezależny przegląd przyrostów, w tym testu aplikacyjnego #70.
   Wynik poprzednika zachowany, bez anulowania kompletnego przebiegu pushem.
2. Rzeczywisty runtime z docelowym modelem i narzędziami na Windows: przygotowanie zależności,
   poprawka błędu kompilacji oraz regresji logicznej, STOP, wznowienie i odtworzenie raportu.
3. Na RTX2060 12GB: pamięć i czasy przy 8k kontekstu oraz obciążeniu głosu/ekranu. Po tym
   próbki większych repozytoriów dla wszystkich przypisanych do kodowania person.
4. Kontrolowana instalacja zwykłego pakietu i osobna próba administratora/sterownika na maszynie
   testowej Windows, z odtwarzaniem stanu. Nie uznajemy samych zgód Brokera za instalację.
5. Prawdziwe API/hybryda po skonfigurowaniu znanego konta; #41/#48 nadal wymagają pełnych prób
   semantyki i faktów wykonania. Dostawca WWW i podpisy ADR dla kontroli telefonu pozostają otwarte.

[Raport kodowania](../reviews/2026-11-10-coding-workflow.md) ·
[Telefon i kokpit](56-kokpit-jarvis-i-panel-telefonu.md)
