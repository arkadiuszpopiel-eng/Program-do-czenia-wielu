# Domykanie usterek — raport i dowody, 10.11.2026

Ostatni pełny odbiór: b85c5371 ma CI186 **10/10** i Windows109 **4/4**. Nowy69e33656 rozwija8116740d: zegar generacji widzi niepuste delty przed buforującą kopertą, a TTFT/after_output nadal wymagają zdekodowanego wyjścia. Pingi/metadane nie zmieniają deadline. Limity60/30s i zachowanie chmury bez zmian. [Raport #77](2026-11-10-tool-envelope-progress.md). Wynik następnego heada jest dopisywany w [PR24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24). Nie jest to deklaracja ukończonej jakości Enterprise ani odbioru sprzętowego.

## Naprawione zakresy kodu

| Karta | Zmiana i dowód lokalny |
| --- | --- |
| #64 | Gotowy wynik HTTP/SearchPort po istniejącym deadline jest odrzucany; Stop pozostaje pierwszy. RED0/6 → 68/0/0, Clippy0. |
| #73 | Końcowy raport zachowuje odmowę Policy z trwałego stanu runtime także po wznowieniu. RED1/2 → 135/0/0, Clippy0. Dodatkowa integracja RunSpec→runtime→tekst rozmowy: RED0/1 → 1/0, obejmuje Alfę, Betę, Gamę, Deltę i własną Novę. Model/narzędzie w tej integracji są skryptowane. |
| #75 | Pomiar zakończenia procesu nie uznaje błędu odczytu /proc za zakończenie i nie wykonuje sprzecznego drugiego odczytu. RED1/3 → 59/0/1, Clippy0; 20/20 powtórzeń ×3 mosty-atrapy przy niezmienionym limicie2s. |
| #76 | Fixture instalacji publikuje niezmienną binarkę Cargo przez dowiązanie twarde w tym samym systemie plików. Nie otwiera wykonywanego pliku do zapisu. 80/0/0, Clippy0 i20/20 powtórzeń po8 istniejących przypadków. Produkcyjny launcher i installer niezmienione. |
| #70 | Test sprawdza pełną treść wejścia i base64; cztery cyfry4417 mogły pochodzić z legalnych metadanych. Dokładna wcześniejsza łatkaSHA256636b5802… ma blob41db3161…. Poprzednio5/5 iClippy0 na b4e2dbcd; lokalny przebieg przerwany przed wynikiem. Aktualny test computer i prywatność terminala przeszły wCI184 na Linux i Windows. |

Liczby modułów oznaczają zaliczone/niezaliczone/ignorowane; RED oznacza zaliczone/niezaliczone. Powtórzeń nie doliczamy jako nowych unikalnych testów.
Pierwszy fixture #73 używał błędnego pola text zamiast content (0/3); błąd przygotowania poprawiono i zachowano, a właściwy reproduktor dał RED1/2. Twarda odmowa już wcześniej przerywała przebieg przed następną odpowiedzią modelu.
#64 nie cofa wysłanego żądania ani rozpoczętego zewnętrznego I/O. #73 dodaje fakt odmowy z wykonania, nie potwierdza prawdziwości wszystkich zdań modelu.

## Pełne CI i zachowane porażki

[CI186](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42912970574) na b85c5371: **10/10**, Linux **3600/0/27**, Windows **3622/0/48**, po1143 wyników celów; Vitest **573**, Chromium **945**, Tauri **5+4**, Clippy/fmt/deny/schematy/graf/docs sukces.
[Windows109](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42912970490): **4/4**, WebView2 **63/0/5**. Qwen8B CPU:15272ms pierwszy token,28100ms/96tokenów/7,48tok/s. Bielik4.5B:12071ms/25754ms/82tokeny/5,99tok/s;11B NOT_RUN_CPU_BUDGET. PiperTTFB849ms, nagranieSTT WER0/40423ms. [Wyniki i hashe](artifacts/ci186-complete.json).
Następny69e33656 ma lokalnie163/0/1,Clippy/fmt/lint0 i wymaga własnego CI.

[CI185](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42892472297) na46f515bf: **10/10**, Linux **3595/0/26**, Windows **3617/0/47**, po1141 wyników celów, Vitest **573**, Chromium **315+315+315=945**. Tauri **5+4**, Clippy/format/deny/schematy/graf/docs zielone.
[Windows108](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42892472263): **4/4**, WebView2 **63/0/5**; Qwen8B CPU **11738ms** pierwszy token, **25387ms/96tokenów/7,03tok/s**. Bielik4.5B CPU12089ms/24041ms/71tokenów/5,94tok/s;11B **NOT_RUN_CPU_BUDGET**,0tokenów. PiperTTFB838ms; nagranie STT WER0,40672ms. To fixture audio, nie prawdziwy mikrofon. [Wyniki i SHA256 zdekodowanych logów](artifacts/ci185-complete.json); logi są kopią UTF8 z normalizacją CRLF→LF, nie bajtami archiwum ZIP.

[CI184](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42884779747) na77a85caa: **10/10**, Linux **3595/0/26**, Windows **3617/0/47**, po1141 wyników celów. Vitest573, Chromium315+315+315=945, format/Clippy/deny/Tauri/docs zielone.
[Windows107](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42884779841): **4/4**, WebView2 **63/0/5**. Qwen8B CPU:20856ms do pierwszego tokena,35391ms/96tokenów/6,6tok/s. Bielik4.5B CPU:16312ms/36307ms/93tokeny/4,65tok/s. Bielik11B: NOT_RUN_CPU_BUDGET,0tokenów (RAM8192MB/VRAM0MB). Tool-called w tej sondzie oznacza propozycję modelu, nie fizyczną akcję.

[CI182](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42854928717) na574b3391: **9/10**, Linux3580/1/26 (#75), Windows3607/0/47. Porażka po4,63s przy budżecie20s dowodzi sprzeczności kolejnych odczytów, ale brak surowego /proc nie pozwala ustalić konkretnego errno lub historycznego stanu PID.
[CI183](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42863147933) na632e5d43: **9/10**, Linux3593/1/26 (#76), Windows3616/0/47, po1140 wyników celów. Test install_after_start:2/1/0, errno26 Text file busy. Nie zidentyfikowano historycznego uchwytu; osobny reproduktor potwierdził błąd przy otwartym własnym uchwycie zapisu i poprawny start po jego zamknięciu.
W CI183: Vitest573, Chromium315+315+315=945, Tauri, cargo-deny i dokumentacja zielone. Nie ponawiano tych przebiegów dla zieleni i nie przerwano ich nowym pushem.

[Windows106](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42863148129): **4/4**, WebView2 **63/0/5**. Pięć pominięć nie jest zaliczeniem. Qwen8B CPU: pierwszy token21722ms,96tokenów,41993ms,4,74tok/s. Bielik11B: NOT_RUN_CPU_BUDGET,0tokenów przy RAM8192MB/VRAM0MB. Propozycja narzędzia w sondzie nie dowodzi fizycznego wykonania akcji.

## Rzeczywisty Qwen8B przez Core

Pinned Qwen3-8B Q4_K_M,4wątkiCPU,96tokenów, temperatura0. Limity60s pierwszej odpowiedzi,30s idle i120s tury nie zostały podniesione. Lokalny BrainPort wymuszał dostawcę; to nie odbiór automatycznej hybrydy/API.
- Przed restartem Cloud: **21 poprawnie zakończonych odpowiedzi,1 błąd Bety/Jarvisa po63738ms i18 niewykonanych**. W20 odpowiedziach Standard obejrzano zgodne imiona i jedno zdanie. Nie rozciągamy tego na całą jakość wszystkich person.
- Surowy replay Bety/Jarvisa: headers10ms, po30053ms jedynie heartbeat3bajty `:\n\n`, następnie idle, bez treści. Serwer zakończono.
- Po restarcie, kontrola tego samego dotychczasowego kodu: **40/40** niepustych odpowiedzi bez błędu, prawidłowy autor i znacznik profilu w żądaniu. Po odzyskaniu Cloud o19:33UTC sprawdzono wszystkie teksty: **40/40 poprawnych imion, gotowości do pomocy i jednego zdania**. To wąska ocena przedstawienia się, bez deklaracji ogólnej jakości zadań lub naprawienia wag1.5B.
- Kandydat zmiany kolejności instrukcji też uzyskał40/40, lecz został **odrzucony**. Suma czasów488970→486770ms, mediana6187→6316,5ms, peakRSS9636452→10090908KiB. Jedna rozgrzana Beta zwolniła4471→34820ms. Jednostkowe37/0/0 i integracje15/0/2ignorowane należą do odrzuconego eksperymentu, nie do końcowej zmiany produktu.

Dotychczasowy prompt, semantyka timeoutów HTTP, wagi i polityki pozostają w wersji produktu. Sukces kontroli po restarcie nie dowodzi naprawienia wcześniejszego timeoutu ani odbioru RTX2060 12GB. RSS CPU nie jest pomiarem VRAM.
Próba faktów i własnej Luny nie zostały wykonane przed drugą utratą Cloud. Po odzyskaniu środowiska o19:33UTC wznowiono kolejno brakujące próby, z tym samym przyjętym kodem i parametrami. Wyniki końcowe: wbudowane40/40 + Luna10/10 poprawnych tożsamości i jednego zdania; oddzielnie40/40 +10/10 zgodnych odpowiedzi o braku działań w świeżych sesjach. Oceniono wszystkie teksty, zachowano żądania i surowe odpowiedzi. Luna zachowuje katalog/obsadę po restarcie bez ponownego przydzielania ról. To nie rozwiązuje szerszego #58: zapisane stany wcześniejszych narzędzi nadal nie trafiają do kontekstu modelu. Deterministyczny reproduktor dał RED0/1. Nowy kandydat V1 mimo app-chat48/0/0 i właściwych integracji Core nie poprawił modelu:30/30 transportu, **0/30 wierności rejestrowi** (20 niepopartych potwierdzeń,10 pominięć Done). **Odrzucony; nie jest kodem aplikacji.** Jedna odpowiedź ma110248ms do widocznego tekstu od startu Core. [Pełne dane, diagnoza i łatka](2026-11-10-history-receipts.md).

## Większy model — rzeczywista próba11B

Oficjalny Bielik11B Q4_K_M pobrano i sprawdzono:6724050496bajtów, SHA256ece69888….
Na Cloud z limitem cgroup16GiB i4CPU model załadował się53,722s. Pierwsze suroweHTTP
ze sprzeczną historią nie zwróciło tekstu w120,014s: **0odpowiedzi/1timeout/2niewykonane**.
Jakość **NOT_EVALUATED**, a katalog nie deklaruje tool calling. Nie jest to próbaCore/GPU,
nie porównujemy tego z0/30 semantyki Qwena. [Pełny protokół i dane](2026-11-10-bielik11b-cpu.md).
Główny8B i pomocniczy1.5B pozostają zainstalowane. Nieużywany cache4.5B i dwa odtwarzalne
pliki testowe usunięto z manifestem. Po próbie mały zapas dysku zwiększono bezstratną
kompresją zamrożonych binarek; po dekompresji sprawdzono oryginalneSHA256.
Logi i źródła zachowane; przed ponowieniem starego kontrolera trzeba przywrócić plik z gzip.

## Korekta zakresu #70

Wcześniej zbyt szeroko uznano zwykły test app-core za chroniony moduł Jądra. [AGENTS:94](../../AGENTS.md) chroni crates/core-*, Broker/watchdog/updater i polityki; [architektura](../ARCHITECTURE.md) oraz [crates/README](../../crates/README.md) odróżniają aplikacyjne app-core. Łatka zmienia wyłącznie asercje testu, bez polityk i tagów prywatności. Zlecenie napraw wystarcza do tego commitu; poprzednie pytanie o dodatkową zgodę było zbędne.
Nie rozszerza to wąskich zgód Brokera/SQLCipher. Standardowy niezależny przegląd i zielonyCI przed merge pozostają wymagane. PR pozostaje draft, bez merge.

## Format danych przeglądu

Przy publikacji b85c5371 `git diff --cached --check` zwrócił2 dla pustych linii kontekstu
w plikach unified patch; skrypt mimo tego kontynuował commit/push. Ten wynik pozostaje
niezaliczony, a błąd kolejności skryptu został zgłoszony właścicielowi. Następny lokalny
przyrost zmienia tylko format łatki na unified0, z jawnym `git apply --unidiff-zero`.
Sprawdzono odtworzenie dokładnych drzew/kodu w osobnym tymczasowym indeksie Git, bez
modyfikacji aplikacji. [SHA256 i sprawdzenia](artifacts/patch-format-validation.json).
PełneCI186 zakończyło się przed publikacją korekty; nie przerwano przebiegu.

## Zapis i pozostały backlog

Zweryfikowano i zamknięto **49 kart (41 starszych,6 obecnych napraw oraz próby #31 i #43)** z naprawami w historii oraz dowodami CI181/Windows104. Zamknięcie dotyczy zakresu kodu w gałęzi, nie wydania lub odbioru sprzętu. Naprawy #64/#70/#73/#74/#75/#76 mają własny pełny CI184 i zostały zamknięte. Kartę #43 zamknięto po rzeczywistej próbie Luny10+10 na aktualnym głównym8B, z zachowaniem historycznych porażek1.5B. Kartę #31 zamknięto w wąskim zakresie poprawnych imion i jednego zdania na aktualnym głównym8B:50/50 rzeczywistych odpowiedzi; nie jest to naprawa wag pomocniczego1.5B. Otwarte pozostają #22/#41/#48/#58/#68/#69/#77/#78; wymagają jakości dłuższej rozmowy, pełnego odbioru lub realizacji brakującego transportu telefonu.
Cloud zrestartował się o18:16UTC; pliki przetrwały, nieukończoną kompilację zapisano osobno i wznowiono. O18:54UTC exec-server utracił transport, a wybrane środowisko stało się niedostępne. Po odzyskaniu o19:33UTC pozostały logi i procesy zombie, bez aktywnego cargo/modelu oraz bez wyniku testu70. Ten lokalny przebieg nie jest zaliczeniem; aktualny kod sprawdza pełne CI184.
Sprawdzone lokalne commity odzyskano z bundle7bf2069f…; zweryfikowano checksum packa,40 obiektów, znane SHA commitów oraz identyczność drzew testowanych plików. Dzięki temu publikacja przez GitHub nie wymagała odtwarzania kodu z pamięci.
- **Nieodebrane:** rzeczywisty RTX2060 12GB, telefon, mikrofon, UAC i instalacja sterowników; nie deklarujemy płynności na tym sprzęcie.
- **WWW #78:** SearchPort nie jest produkcyjną wyszukiwarką. Wybór dostawcy pozostaje otwarty; nie czytano kluczy i nie wysyłano zapytań do przypadkowych usług.
- **Telefon #69:** panel pozostaje podglądem; brama/parowanie i pełne sterowanie nie są zaimplementowane. #68 obejmuje dodatkowo odbiór mobilny.
- **Modele:** Bielik1.5B jest pomocniczy; wcześniejsze50/50 fikcyjnych potwierdzeń nie znikają z historii. Główny jest8B,11B tekstową alternatywą wymagającą odbioru.
- **Weryfikacja:** nie ma nowego prawdziwegoAPI, GPU lub mikrofonu. Nie uruchamiano prawdziwych CLIbridges ani subagentów, harmonogramów nie wznawiano.
- **Dokumentacja:** po odzyskaniu Cloud uaktualniono roadmapę i plan55, w tym błędną kwalifikację #70. HTML jest generowany narzędziem, bez ręcznych zmian.

[Komendy i dane](artifacts/defect-closure-final-state.json) · [Zdekodowana kopia22 odpowiedzi niezaliczonej próby](artifacts/core-qwen-initial-22.json) · [Bundle źródeł do odtworzenia](artifacts/defect-closure-candidates.bundle). Bundle zawiera również odrzuconą gałąź eksperymentu, której nie włączono do aplikacji.
[Kontrola40](artifacts/core-qwen-control-40.json) · [Fakty40](artifacts/core-qwen-facts-40.json) · [Luna10 tożsamość](artifacts/core-qwen-luna-identity-10.json) · [Luna10 fakty](artifacts/core-qwen-luna-facts-10.json). Osobne pliki -review i -status zawierają ocenę i czasy/hashe.
Surowe logi lokalne pozostają w `/workspace/scratch/defect-closure/` ze wskazanymi SHA256. Checkpoint odnowiono po odzyskaniu Cloud; lokalny feat doprowadzono fast-forward do77a85caa. Odrzucony #58 zachowano lokalnie w osobnym worktree i w opublikowanej łatce; nie włączono do aplikacji. Kandydat samej kadencji #77 został odrzucony; nie ma flagi ping10 w produkcji. Przyjęty lokalnie8116740d wybiera limity według postępu treści, z domyślną dotychczasową semantyką adapterów chmurowych; bez zmiany polityk/promptu. Wymaga własnego CI.
