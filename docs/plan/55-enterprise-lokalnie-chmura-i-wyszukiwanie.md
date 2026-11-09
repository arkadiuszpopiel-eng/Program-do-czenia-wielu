# 55 — niezawodność lokalnie, w chmurze i wyszukiwanie WWW

**Kandydat[#57](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/57):**
propozycja narzędzia wymagająca potwierdzenia zachowuje osobny stan w rozmowie i Replay,
bez oznaczania jako wykonana. [Wyniki lokalne i ograniczenia](../reviews/2026-11-10-tool-confirmation.md).
Bazaaa2e75ff ma zieloneCI171 iWindows93;nowy przyrost wymaga osobnegoCI i przeglądu.


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
