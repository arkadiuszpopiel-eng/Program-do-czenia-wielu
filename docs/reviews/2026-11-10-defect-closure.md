# Domykanie usterek — raport i dowody, 10.11.2026

Stan zapisany przed końcowym CI. Aktualny wynik ostatniego heada jest dopisywany w [PR24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24). Nie jest to deklaracja ukończonej jakości Enterprise ani odbioru sprzętowego.

## Naprawione zakresy kodu

| Karta | Zmiana i dowód lokalny |
| --- | --- |
| #64 | Gotowy wynik HTTP/SearchPort po istniejącym deadline jest odrzucany; Stop pozostaje pierwszy. RED0/6 → 68/0/0, Clippy0. |
| #73 | Końcowy raport zachowuje odmowę Policy z trwałego stanu runtime także po wznowieniu. RED1/2 → 135/0/0, Clippy0. Dodatkowa integracja RunSpec→runtime→tekst rozmowy: RED0/1 → 1/0, obejmuje Alfę, Betę, Gamę, Deltę i własną Novę. Model/narzędzie w tej integracji są skryptowane. |
| #75 | Pomiar zakończenia procesu nie uznaje błędu odczytu /proc za zakończenie i nie wykonuje sprzecznego drugiego odczytu. RED1/3 → 59/0/1, Clippy0; 20/20 powtórzeń ×3 mosty-atrapy przy niezmienionym limicie2s. |
| #76 | Fixture instalacji publikuje niezmienną binarkę Cargo przez dowiązanie twarde w tym samym systemie plików. Nie otwiera wykonywanego pliku do zapisu. 80/0/0, Clippy0 i20/20 powtórzeń po8 istniejących przypadków. Produkcyjny launcher i installer niezmienione. |
| #70 | Test sprawdza pełną treść wejścia i base64; cztery cyfry4417 mogły pochodzić z legalnych metadanych. Dokładna wcześniejsza łatkaSHA256636b5802… ma blob41db3161…. Poprzednio5/5 iClippy0 na b4e2dbcd; wynik aktualnego lokalnego polecenia nieodczytany po utracie Cloud. Bieżącą bazę musi potwierdzić końcowyCI. |

Liczby modułów oznaczają zaliczone/niezaliczone/ignorowane; RED oznacza zaliczone/niezaliczone. Powtórzeń nie doliczamy jako nowych unikalnych testów.
Pierwszy fixture #73 używał błędnego pola text zamiast content (0/3); błąd przygotowania poprawiono i zachowano, a właściwy reproduktor dał RED1/2. Twarda odmowa już wcześniej przerywała przebieg przed następną odpowiedzią modelu.
#64 nie cofa wysłanego żądania ani rozpoczętego zewnętrznego I/O. #73 dodaje fakt odmowy z wykonania, nie potwierdza prawdziwości wszystkich zdań modelu.

## Dwie zachowane porażki CI

[CI182](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42854928717) na574b3391: **9/10**, Linux3580/1/26 (#75), Windows3607/0/47. Porażka po4,63s przy budżecie20s dowodzi sprzeczności kolejnych odczytów, ale brak surowego /proc nie pozwala ustalić konkretnego errno lub historycznego stanu PID.
[CI183](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42863147933) na632e5d43: **9/10**, Linux3593/1/26 (#76), Windows3616/0/47, po1140 wyników celów. Test install_after_start:2/1/0, errno26 Text file busy. Nie zidentyfikowano historycznego uchwytu; osobny reproduktor potwierdził błąd przy otwartym własnym uchwycie zapisu i poprawny start po jego zamknięciu.
W CI183: Vitest573, Chromium315+315+315=945, Tauri, cargo-deny i dokumentacja zielone. Nie ponawiano tych przebiegów dla zieleni i nie przerwano ich nowym pushem.

[Windows106](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42863148129): **4/4**, WebView2 **63/0/5**. Pięć pominięć nie jest zaliczeniem. Qwen8B CPU: pierwszy token21722ms,96tokenów,41993ms,4,74tok/s. Bielik11B: NOT_RUN_CPU_BUDGET,0tokenów przy RAM8192MB/VRAM0MB. Propozycja narzędzia w sondzie nie dowodzi fizycznego wykonania akcji.

## Rzeczywisty Qwen8B przez Core

Pinned Qwen3-8B Q4_K_M,4wątkiCPU,96tokenów, temperatura0. Limity60s pierwszej odpowiedzi,30s idle i120s tury nie zostały podniesione. Lokalny BrainPort wymuszał dostawcę; to nie odbiór automatycznej hybrydy/API.
- Przed restartem Cloud: **21 poprawnie zakończonych odpowiedzi,1 błąd Bety/Jarvisa po63738ms i18 niewykonanych**. W20 odpowiedziach Standard obejrzano zgodne imiona i jedno zdanie. Nie rozciągamy tego na całą jakość wszystkich person.
- Surowy replay Bety/Jarvisa: headers10ms, po30053ms jedynie heartbeat3bajty `:\n\n`, następnie idle, bez treści. Serwer zakończono.
- Po restarcie, kontrola tego samego dotychczasowego kodu: **40/40** niepustych odpowiedzi bez błędu, prawidłowy autor i znacznik profilu w żądaniu. To test transportu/routingu; nie wykonano pełnej końcowej oceny semantycznej wszystkich40 tekstów.
- Kandydat zmiany kolejności instrukcji też uzyskał40/40, lecz został **odrzucony**. Suma czasów488970→486770ms, mediana6187→6316,5ms, peakRSS9636452→10090908KiB. Jedna rozgrzana Beta zwolniła4471→34820ms. Jednostkowe37/0/0 i integracje15/0/2ignorowane należą do odrzuconego eksperymentu, nie do końcowej zmiany produktu.

Dotychczasowy prompt, semantyka timeoutów HTTP, wagi i polityki pozostają w wersji produktu. Sukces kontroli po restarcie nie dowodzi naprawienia wcześniejszego timeoutu ani odbioru RTX2060 12GB. RSS CPU nie jest pomiarem VRAM.
Końcowa próba pytań o fakty wykonania oraz Qwen z własną Luną po restarcie nie zostały wykonane przed kolejną utratą Cloud. #31/#41/#43/#48/#58 nie są zamykane samym kodem0 transportu.

## Korekta zakresu #70

Wcześniej zbyt szeroko uznano zwykły test app-core za chroniony moduł Jądra. [AGENTS:94](../../AGENTS.md) chroni crates/core-*, Broker/watchdog/updater i polityki; [architektura](../ARCHITECTURE.md) oraz [crates/README](../../crates/README.md) odróżniają aplikacyjne app-core. Łatka zmienia wyłącznie asercje testu, bez polityk i tagów prywatności. Zlecenie napraw wystarcza do tego commitu; poprzednie pytanie o dodatkową zgodę było zbędne.
Nie rozszerza to wąskich zgód Brokera/SQLCipher. Standardowy niezależny przegląd i zielonyCI przed merge pozostają wymagane. PR pozostaje draft, bez merge.

## Zapis i pozostały backlog

Zweryfikowano i zamknięto **41 starszych kart** z naprawami w historii oraz dowodami CI181/Windows104. Zamknięcie dotyczy zakresu kodu w gałęzi, nie wydania lub odbioru sprzętu. Nowe naprawy oczekują własnego końcowegoCI; ich stan aktualizuje PR24.
Cloud zrestartował się o18:16UTC; pliki przetrwały, nieukończoną kompilację zapisano osobno i wznowiono. Później exec-server utracił transport, a wybrane środowisko stało się niedostępne. Wynik rozpoczętego testu70 i zakończenie jego procesu cargo są obecnie niepotwierdzone.
Sprawdzone lokalne commity odzyskano z bundle7bf2069f…; zweryfikowano checksum packa,40 obiektów, znane SHA commitów oraz identyczność drzew testowanych plików. Dzięki temu publikacja przez GitHub nie wymagała odtwarzania kodu z pamięci.
- **Nieodebrane:** rzeczywisty RTX2060 12GB, telefon, mikrofon, UAC i instalacja sterowników; nie deklarujemy płynności na tym sprzęcie.
- **WWW:** SearchPort nie jest produkcyjną wyszukiwarką. Wybór dostawcy pozostaje otwarty; nie czytano kluczy i nie wysyłano zapytań do przypadkowych usług.
- **Telefon #69:** panel pozostaje podglądem; brama/parowanie i pełne sterowanie nie są zaimplementowane. #68 obejmuje dodatkowo odbiór mobilny.
- **Modele:** Bielik1.5B jest pomocniczy; wcześniejsze50/50 fikcyjnych potwierdzeń nie znikają z historii. Główny jest8B,11B tekstową alternatywą wymagającą odbioru.
- **Weryfikacja:** nie ma nowego prawdziwegoAPI, GPU lub mikrofonu. Nie uruchamiano prawdziwych CLIbridges ani subagentów, harmonogramów nie wznawiano.
- **Dokumentacja:** roadmapa i plan55 wskazują ten raport. Po utracie Cloud nie zmieniano planów ani generowanychHTML; ich dotychczasowe103strony musi sprawdzić końcowyCI. Uzupełnienie wpisów inline/generatora wymaga dostępnego środowiska.

[Komendy i dane](artifacts/defect-closure-final-state.json) · [Zdekodowana kopia22 odpowiedzi niezaliczonej próby](artifacts/core-qwen-initial-22.json) · [Bundle źródeł do odtworzenia](artifacts/defect-closure-candidates.bundle). Bundle zawiera również odrzuconą gałąź eksperymentu, której nie włączono do aplikacji.
Surowe logi i pełne40-odpowiedziowe raporty pozostają w `/workspace/scratch/defect-closure/` ze wskazanymi SHA256. Po odzyskaniu Cloud najpierw sprawdzić procesy, checkpoint, git i aktualny publiczny head; lokalny feat może nadal wskazywać632e5d43.
