# Naprawy obecnych usterek i próby Bielika, 9.11.2026

Gałąź: `feat/nightly-modes-jarvis`, draft [PR #24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24).
Korekta właściciela z 06:53 UTC zastąpiła dawny termin: tylko obecne usterki, bez nowych funkcji.
Po rozliczeniu wyników i zapisie koniec prac do odwołania. Automatyzacja nocna pozostaje wyłączona.
To raport autorski, nie wymagany niezależny przegląd.

**Cztery usterki mają sprawdzone naprawy. #41/#48 nadal nie są usunięte.**
NoweCI167 wykryło ponadto podatność używanego SQLCipher/SQLite:
[RUSTSEC-2026-0459 — dowody i blokada audytu](2026-11-09-ci167-sqlite.md).
Nie uznajemy całego zadania za zakończone. Audyt historycznegoCI167 był czerwony;
po odrębnej naprawie i zgodzie na dokładną łatkę licencji aktualny audyt4/4,
pełneCI168 wszystkie6 zielone iWindows90 wszystkie4 zielone na050f86c7.
Zmiany instrukcji modelu nie przeszły jakości i nie trafiają do kodu produkcyjnego.
[Kompletna odrzucona łatka](patches/jarvis-model-identity-candidate.patch),
[walidacja i odciski źródeł](artifacts/jarvis-candidate-validation.json).
Łatka dotyczy dokładnie bazy cc6f0ac9, została sprawdzona przez `git apply --check`;
nie jest zatwierdzoną poprawką do automatycznego zastosowania.

## Cztery opublikowane naprawy

Baza `cc6f0ac994185387c348ba60ab6edf88908cd28e` obejmuje:

| Usterka | Naprawa | Dowód |
| --- | --- | --- |
| #53 | etykiety PL/EN system/vision i kolor w Kreatorze | Chromium 905/905; Windows/WebView2 63/0/5; obraz Windows obejrzany |
| #37 | append-only zapis i odtworzenie obsady rozmowy po restarcie | regresja brakującej roli Luny RED → 76/76 modułów i 17/17 integracji |
| #51 | importy chronionego testu Brokera pod cfg(windows) | dokładna łatka zatwierdzona przez właściciela „ok”; pełny Clippy bez ostrzeżeń, 4/4 |
| #52 | anulowalne odstępy 250/500/1000 ms po chwilowym błędzie HTTP | RED 3/1 → 4/4; app-models 82/0/1; bez większej liczby żądań lub timeoutów |

[CI166](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42592814337)
na tej bazie: wszystkie 6 zadań zielone. Rust Linux 3504/0/25, Rust Windows 3530/0/46,
po 1125 oryginalnych wyników celów; Clippy 0. Chromium 905/905 w 27,8 min, Vitest 573/573
w 103 plikach, typy 0 błędów/0 ostrzeżeń; lint/build/budżet/Storybook/Tauri/cargo-deny/dokumentacja zaliczone.

[Windows rehearsal87](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42592814476):
4 zadania zielone; WebView2/CDP63/0/5 w2,8min; silnikiCPU 1/0/2 filtrowane w 520,58 s.
Syntetyczny Piper→Whisper: WER 0, STT 42679 ms, RTF 19,43. Poprzednie awarieHTTP504
w rehearsal83/85 pozostają zapisane; odstępy nie gwarantują dostępności serwera zewnętrznego.
Są to wyniki bazy, nie dowód nowej zmiany instrukcji modelu.

## Odrzucony kandydat instrukcji #41/#48

- Domyślny szablon poprawia gramatykę opisu persony. Pięć wspólnych zasad pozostaje identycznych.
- Standard i Jarvis dostają imię oraz narzędnik z rzeczywistego katalogu, również własnych person.
  Instrukcja nie zawiera gotowego meldunku gotowości ani odpowiedzi na pytanie testowe.
- Opisy ról opisują pracę jednej agentki; nie są scenariuszem dialogu do kontynuowania.
  Meldunek o wykonaniu musi wynikać z rzeczywistego wyniku.
- Fakt o braku narzędzi jest dopisany dopiero do końcowego żądania, po wyborze modelu i narzędzi.
  Nie jest dodawany do żądania zawierającego narzędzia. Zdolności modeli i uprawnienia są dotychczasowe.
- Przy niewybranym projekcie i katalogu pominięto powielony tytuł i instrukcje wybranego projektu.
  Pozostają fakty i ograniczenie odczytu/edycji. Wybrany cel zachowuje pełną dotychczasową instrukcję.
- Ręczna próba zapisuje końcowe żądanie faktycznego dostawcy. Luna ma 10 rozmów i przypisane role
  przed restartem; po restarcie role nie są ręcznie przypisywane ponownie.

## Odrzucone warianty i próba kontrolna

Wariant 43 dał 50/50 poprawnych krótkich przedstawień na 1.5B:5 powtórzeń ×5 person ×2 profile
(Standard i Jarvis). Autorka, kanoniczne imię, jedno zdanie, gotowość pomocy, brak błędu/obcięcia
oraz narzędzi były sprawdzone osobno. **To nie dowodzi ogólnej zgodności formatu.**
Szersza próba została przerwana po5/28: wszystkie 5 odpowiedzi na polecenie podania samej liczby
zawierało także imię. Wariant44 z dodatkowym warunkiem odrzucono po 2/38 z tym samym problemem;
nieuruchomionych przypadków nie zaliczono. Zatrzymane procesy zakończyły się 130.

[Surowe50 odpowiedzi43/1.5B](artifacts/jarvis-identity-v43-1.5b.jsonl) ·
[Niezaliczone próby formatu](artifacts/jarvis-format-regressions.jsonl) ·
[Indeks prób iSHA256](artifacts/jarvis-identity-attempt-index.jsonl).

**Próba kontrolna:** opublikowana instrukcja także nie spełniła polecenia samej liczby:
Alfa zmyśliła przekazanie Delcie i „Gotowe”, Beta dopisała imię. Obie odpowiedzi na prośbę
przedstawienia się zawierały więcej niż jedno zdanie. Nie przypisujemy tych wad wyłącznie
nowemu wariantowi. Prostszy opis katalogowej tożsamości dał w tej małej próbie 4/4:
Alfa/Standard iBeta/Jarvis podały samo12 oraz osobno jednozdaniowe przedstawienie się.
[Osiem kompletnych żądań i odpowiedzi](artifacts/jarvis-format-controlled-comparison.jsonl).
To kontrola diagnostyczna, nie pełna integracja ani niezależny przegląd.

### Bielik 4.5B i limitCPU

Normalne integracje wcześniejszych wariantów kończyły się błędem czasu pierwszej odpowiedzi
około 64 s. Wariant 43 z dodatkowym logiem serwera przeszedł dwie odpowiedzi, ale obie miały
po dwa zdania; trzecia, Gamy, zakończyła się timeoutem. TestRust: 0/1, zapisano 3 z planowanych 40.
Nie jest to zaliczenie 4.5B. [Surowe odpowiedzi i żądania](artifacts/jarvis-identity-v43-4.5b.jsonl).

Log llama.cpp: Alfa 706 tokenów wejścia, prefill 54817,41 ms / 12,88 tok/s, generacja 6329,44 ms;
Beta 559 tokenów, prefill 44567,73 ms / 12,54 tok/s, generacja 9748,39 ms.
Serwer korzystał z 4 wątków i około 310–382% CPU, rdzeń aplikacji około 0–0,5% CPU;
nie wykryto konkurującego aktywnego testu. Ten pomiar nie wskazuje na zajęcieCPU przez rdzeń.
Zwykłe odtworzenie wcześniejszego polecenia 727 tokenów dało pierwszy tekst po 59810 ms,
blisko limitu 60 s. Obserwacja wyjaśnia wrażliwość na czasCPU, nie usprawiedliwia semantycznego błędu.
Nie zwiększono limitu pierwszej odpowiedzi 60 s, bezczynności 30 s ani zewnętrznego oczekiwania 120 s.

## Środowisko i zakres dowodu

Linux Cloud, 4 CPU, brak GPU. Rust 1.94.1, pnpm 10.33, llama.cpp b11559;
kontekst 8192, max_tokens 96, temperature 0. Wagi Q8_0, pełny SHA256 sprawdzony:

- 1.5B: `e206ebbd9ee00cc8f14af5a096ad3f193a59608c63905316e8dc680c96655a60`;
- 4.5B: `562f2291de257890adf2b4a914da8b194affe6a7a838a6b7ef3d342f306c1b7f`.

1.5B nadal ma katalogowe tools=false, 4.5B tools=true. W próbach rozmowy oba otrzymują 0 narzędzi;
nie było rzeczywistego wykonania narzędzi. Pełne piny i oficjalne rewizje modeli są dotychczasowe.
Brak plikuSHA towarzyszącego ręcznie pobranemu 4.5B był błędem przygotowania tej próby;
plik utworzono po pełnym sprawdzeniu wag, bez obejścia walidacji produkcyjnego pobierania.

Dwa profile współpracy i pięć person w próbie modelu nie oznaczają pięciu trybów uprawnień.
Tryby Plan/Ręcznie/Akceptuj edycje/Automatycznie/Pełny dostęp mają osobną regresję UI/Rust.
Zachowano zamrożone evals i ich progi. Liczenie zdań w tej diagnostyce nie zastępuje tych bramek;
nie uznaje się ukończenia transportu za zaliczenie semantyki. Emotikony i Markdown wymagają
osobnej oceny; nie deklarujemy zgodności całej stylistyki na podstawie jednego regexu.

**Brak odbioru Windows 11 właściciela, RTX 2060/6 GB, mikrofonu i jakości odsłuchu.**
Windows/CDP dotyczy debug/dev na runnerze; pięć pominięć pozostaje: profil autonomii,
logowanie CLI, widoczny natywny eksport i dwa skróty głosu. Syntetyczny głosCPU nie jest rozmową
na żywo. Nie uruchamiano mostów CLI, nowych agentów ani nowej automatyzacji.

## Walidacja odrzuconej naturalnej instrukcji

Kod kandydata przeszedł **188/0/0** w pięciu pakietach (36 wyników celów), **12/0/0** integracji
w pięciu celach app-core oraz Clippy app-chat/personas-contract/app-core --all-targets bez ostrzeżeń.
Dwie stare asercje testowe dotyczące wielkiej litery w usuwanym wzorcu przerwały pierwsze
ponowienie (app-chat 41/2); po dostosowaniu do katalogowych form pełne powtórzenie jest zielone.
To nie był błąd wykonania naprawianej funkcji; log pierwszej próby pozostaje zachowany.

Rzeczywiste 1.5B: **50/50 poprawne imię**, **50/50 gotowość pomocy**, **19/50 jedno zdanie**
(Standard 9/25, Jarvis 10/25), **0 błędów i 0 obcięć**. Wszystkie 50 odpowiedzi mają właściwą
autorkę i faktyczne żądanie bez narzędzi. Luna zachowała role po restarcie, bez ponownego przypisania.
Manualne testy Rust zaliczyły transport; osobny kontroler semantyki zakończył się kodem 2:
**nie jest to pełna naprawa #48**.

Najnowsza integracja 4.5B zakończyła się kodem 101: Alfa/Standard, pierwsza odpowiedź,
**64074 ms i timeout**, bez tekstu. Pierwsza z planowanych 50 tur niezaliczona, pozostałe 49
nieuruchomione. Nie porównujemy tego z50 ukończonymi odpowiedziami 1.5B jako równych prób.
Nadal nie można zaliczyć 4.5B przy dotychczasowych limitach na tym CPU.

Pełna dodatkowa próba 1.5B: **14/28 zaliczonych**, bez błędu transportu i bez obcięcia:

| Polecenie | Zaliczone | Błąd |
| --- | --- | --- |
| samo 7 + 5 | 9/10 | Luna/Standard zwróciła 5 |
| czy wykonano narzędzia/pliki | 4/10 | 6 zmyślonych potwierdzeń w nowej rozmowie bez narzędzi |
| opis kopii zapasowej w 2 zdaniach | 0/6 | odpowiedzi mają 3 lub 4 zdania |
| dokładnie „Cześć!” | 1/2 | Beta/Jarvis dopisała pytanie |

[28 kompletnych żądań i odpowiedzi](artifacts/jarvis-candidate-broader-1.5b.jsonl) ·
[sprawdzenie treści wszystkich 28 przez autora, nie niezależną recenzentkę](artifacts/jarvis-candidate-broader-review.json) ·
[50 odpowiedzi Core 1.5B](artifacts/jarvis-candidate-v45-1.5b.jsonl) ·
[niezaliczona pierwsza tura 4.5B](artifacts/jarvis-candidate-v45-4.5b.jsonl).

Zgodność z instrukcją nie jest egzekwowana niezawodnie przez same zmiany promptu.
Nie obchodzimy tego obniżeniem progów, wzrostem timeoutów, gotową odpowiedzią testową,
przepisywaniem tekstu modelu ani fikcyjnym meldunkiem o wykonaniu. Produkcyjne instrukcje modelu wróciły
do sprawdzonej bazy; cały kandydat jest zapisany jako łatka i surowe wyniki.

## Osobny, bezpieczny przyrost diagnostyki

Publikowana zmiana kodu dotyczy wyłącznie ręcznego testu `app-core/tests/jarvis_live.rs`:

- zachowuje rzeczywiste zdolności dostawcy i zapisuje końcowe żądanie po wyborze modelu/narzędzi;
- zapisuje autora, błędy, obcięcia oraz instrukcję routera osobno od instrukcji dostawcy;
- tworzy10 rozmów Luny i przypisuje role przed restartem; po restarcie nie naprawia obsady ręcznie;
- pobiera kopię przechwyconego żądania przed budowaniemJSON, unikając powtórnego blokowaniaMutex.

Domyślne instrukcje produkcyjne, uprawnienia, katalog modeli i progi pozostają na bazie cc6f0ac9.
Nowa diagnostyka nie zamienia nieudanego wyniku modelu w zaliczenie.
Przyrost przeszedł kompilację ręcznego testu i Clippy app-core --all-targets bez ostrzeżeń.
Rzeczywista próba 1.5B na dotychczasowych produkcyjnych instrukcjach: **1 test zaliczony,
0 nieudanych,1 filtrowany; 54,78 s**, **10/10 tur bez błędów i obcięć**, 0 ponownych przypisań roli.
Standard 5 jest kontrolą przed restartem, Jarvis 5 sprawdza obsadę po restarcie.
Test kontroluje także niezmienność uprawnień i nieaktywny mikrofon.
[Wszystkie 10 tur, końcowe żądania iSHA binarium](artifacts/jarvis-luna-production-diagnostic.jsonl).
Pole `core_restarted` określa zakres każdej tury; nie zaliczamy 10 jako prób po restarcie.
Po wykonaniu poprawiono wyłącznie mylący komentarz o ponownym przypisaniu, bez zmiany wykonania.

Końcowe kontrole lokalne: `cargo fmt --all -- --check`, pełny `pnpm lint`,
rozmiary plików/crate'ów, generowanie i sprawdzenie100 stronHTML bez problemów,
`plan-prac`178 zadań oraz `git diff --check` — zaliczone. Nie zmieniono frontendowego kodu.

Źródła produkcyjne app-chat/personas-contract są identyczne z cc6f0ac9. Nowy pełnyCI dotyczy
przyrostu diagnostyki i dokumentacji; aktualny wynik jest dostępny w sprawdzeniach PR #24.

## Dalsze kroki i granice zamknięcia

1. #41/#48 pozostają otwarte: potrzebny kandydat przechodzący zarówno przedstawienia się,
   jak i inne pytania, bez zmyślania wyników. Nie prowadzić nowych funkcji w tej kolejce.
2. Ocenę jakości modelu oddzielić od wydajnościCPU; 4.5B nadal nie zalicza obecnego limitu.
   RTX 2060/6 GB oraz prawdziwy mikrofon wymagają rzeczywistej próby na sprzęcie właściciela.
3. Cztery sprawdzone naprawy i przyrost diagnostyki wymagają niezależnego przeglądu;
   scalenie wyłącznie przy zielonym CI. Nie scalono PR i nie wydano nowej wersji.
4. Zapis raportu nie oznacza usunięcia wszystkich usterek. Automatyzacja jest wyłączona;
   bez ponownego uruchamiania harmonogramu i bez prac nad nowymi funkcjami.


HistoryczneCI167 miało czerwony audyt; odrębna naprawa SQLCipher po zgodzie właściciela
przechodzi lokalny audyt4/4. Nowy pełnyCI jest rozliczany osobno wPR #24.
Odrębny kandydat używa naprawionego SQLCipher4.20/SQLite3.54.1; potwierdzone testy i pozostająca
decyzja licencji są w raporcie SQLCipher. Właściciel zatwierdził wyłącznie wąską łatkę licencji51e27c10; rzeczywisty audyt po zgodzie4/4.
Nie stosowano niezatwierdzonych zmian polityki.


## Dalsza diagnoza bez publikowania niesprawdzonej zmiany

Kod produkcyjny pozostaje równoważny050f86c7. Publikowana zmiana kodu dotyczy wyłącznie ręcznego
testu: `ALFA_LIVE_QUERY` wybiera syntetyczne pytanie, `ALFA_LIVE_JARVIS_FIRST=1` zaczyna próby
wbudowanych person od zimnego Jarvisa. Domyślne pytanie jest dotychczasowe; każde badanie nadal
ma pięć powtórzeń na profil. Luna zachowuje Standard5 przed iJarvis5 po restarcie, bez ponownego
przypisywania ról. Raport zapisuje także końcowe pytanie faktycznego dostawcy. Nie zmienia to produktu,
uprawnień, zdolności modeli, limitów60/30/120s,96tokenów ani temperatury0.

**Hipoteza46, bezpośredniHTTP, odrzucona:** uproszczono opisy ról dla rozmowy bez narzędzi,
zachowując pięć wspólnych zasad, odczyt/izolację i własną rolę Luny. 1.5B:38 odpowiedzi,
jednozdaniowe przedstawienie9/10, sama prawidłowa liczba8/10, prawdziwa informacja o braku działań6/10,
dwa zdania0/6, dosłowne powitanie2/2. Cztery fikcyjne wykonania, dwa obcięcia limitem.
4.5B:34 odpowiedzi z35 prób/38 planowanych. Jedno zdanie5/10; sama liczba0/10 mimo poprawnego12
w10/10; prawdziwa informacja o braku wcześniejszych działań10/10, ale Alfa dopisuje nieistniejące
przekazanie; dwa zdania0/4, Beta dwukrotnie używa męskiej nazwy roli. Próba35 przerwana po około190,6s,
trzy pozostałe nieuruchomione. Skrypt miał timeout odczytu, a nie łączny deadline120s: to wada procedury,
więc nie jest dowodem akceptacji transportu. Dalsze pomiary wykonano przez rzeczywisty Core z jego limitami.

[SuroweHTTP1.5B](artifacts/jarvis-affordance-1.5b-rejected.jsonl) ·
[SuroweHTTP4.5B i przerwanie](artifacts/jarvis-affordance-4.5b-rejected.jsonl).

**Pusty projekt, próba ograniczenia opóźnień, także nieprzyjęta:**

| Źródło i rzeczywisty Core4.5B | Wynik |
| --- | --- |
| Opublikowany050f86c7,Standard pierwszy | 0/1/1filtrowany;AlfaIdle72332ms,1/40prób,39nieuruchomionych |
| Kandydat1,Standard pierwszy | 40/40bez błędu/obcięcia;test1/0/1filtrowany,657,23s;jedno zdanie4/20Standard i5/20Jarvis |
| Kontrola ręcznie nazwanego tytułu kandydata1 | RED0/1:skrót usuwał fakty o tytule;wariant odrzucony pomimo poprawnego transportu |
| Kandydat2,zachowane wszystkie fakty,zimnyJarvis pierwszy | 0/1/1filtrowany;AlfaIdle65862ms,1/40prób,39nieuruchomionych |

Przed naprawą gałęzi: RED0/1 oczekiwanego skrótu; po korekcie zachowania tytułu52/0/0,format,
rozmiary iClippy zielone. Odbiór rzeczywistego modelu nadal niezaliczony. Przywrócono produkcyjne
`project_context.rs` i test projektu do050f86c7. Nie opublikowano zmiany instrukcji kontekstu.
[Kompletne wejścia/wyjścia i odciski](artifacts/jarvis-context-latency-probes.jsonl) ·
[wycofana łatka kontekstu](patches/jarvis-empty-project-context-withheld.patch).
Łatka ma bazę050f86c7, SHA256 `7f886e16bd655a42c792b8497756fdf8caadbca1d50a799cea9ea24cf589ac31`;
nie stosować jej jako zatwierdzonej naprawy. Nie podniesiono limitów ani progów.

### Końcowe próby 1.5B przez Core

Rzeczywisty Core1.5B na niezmienionym kodzie produkcyjnym050f86c7,96tokenów/temperatura0,
po5powtórzeń dla Alfy,Bety,Gamy,Delty iLuny w Standard/Jarvis: **100/100 odpowiedzi bez błędu transportu
lub obcięcia**, cztery poprawne przebiegi Rust. To nie jest zaliczenie jakości.
Przedstawienie: poprawne kanoniczne imię40/50 (Standard15/25,Jarvis25/25),jedno zdanie10/50
(Standard10/25,Jarvis0/25). Standard Bety/Gamy powtarza „Betta”/„Gamma”.
Pytanie o wykonanie: **50/50 fikcyjnych potwierdzeń** uruchomienia narzędzi lub sprawdzenia plików,
pomimo rzeczywistego0narzędzi i świeżej historii. Dotyczy również Luny5przed/5po restarcie;0ponownych
przypisań roli, pytanie faktycznego dostawcy sprawdzone100/100. Żadna taka czynność nie została wykonana.

Oddzielna próba „7+5,podaj samą liczbę”:22wiersze z50planowanych,obie próbyRust0/1/1filtrowany.
Wbudowane:21/40wierszy,4obcięcia;Alfa/Standard kończy pustym tekstem,19pozostałych nieuruchomionych.
Luna:1/10wierszy,obcięcie,9nieuruchomionych. Nie zaliczać tego badania wynikiem samego HTTP lub metadanych error=null.
[100 kompletnych wejść i odpowiedziCore](artifacts/jarvis-production-1.5b-final-core.jsonl) ·
[niezaliczona arytmetyka](artifacts/jarvis-production-1.5b-arithmetic-failed.jsonl).

PełneCI kodu produkcyjnego050f86c7:Linux3506/0/26,Windows3532/0/47,po1126 celów,Clippy0;
Chromium905/905/29,3min,Vitest573/573/103pliki,Svelte0/0,Tauri9/0/0,cargo-deny4kontrole,
dokumentacja zielone. Windows90:WebView2 63/0/5,syntetyczne silniki1/0/2 filtrowane,WER0.
[Dokładny raport silnika bazy iCI](2026-11-09-ci167-sqlite.md).
Nie oznacza to zamknięcia jakości modelu, przeglądu ani odbioru sprzętu/mikrofonu właściciela.


### Zlecone porównanie8B i nowy sprzęt docelowy

Właściciel poprosił o rzeczywistą próbę modelu8–9B i zmienił cel na **RTX2060 12GB**, zamiast6GB.
Wybrano oficjalny Qwen3-8B Q4_K_M z repozytoriumQwen, rewizja7c41481f57cb95916b40956ab2f0b139b296d974.
Pełne5027783488bajtów zweryfikowano SHA256
`d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785`.
Ręczna próba wczytuje dodatkowy publiczny `ALFA_LIVE_MODEL_ENTRY` z obowiązkowym pinemSHA256;
nie pozwala zastępować modeli wbudowanych ani zmieniać ich zdolności. Produkcyjny katalog i domyślny model
pozostają dotychczasowe. Odrębny plik testowej konfiguracji umożliwia taką próbę bez zmiany polityk.

**Qwen3-8B Q4_K_M — sonda jakości30/30 poprawna**,po jednym przypadku dla każdej z5person
w2profilach i3pytaniach: prawdziwa informacja o braku działań10/10,kanoniczne imię+jedno zdanie+gotowość10/10,
sama prawidłowa liczba10/10.0obcięć,0wywołań narzędzi,nie zwrócono treści rozumowania; trybbezrozumowania
ustawiono oficjalnym `enable_thinking=false`. Porównanie używa96tokenów/temperatury0 i oryginalnych
instrukcji person/ról050f, z odświeżonym syntetycznym tytułem dla pytania; Qwen jest wQ4,Bieliki wQ8.
To jest sonda po1powtórzeniu, nie pełne5powtórzeń ani test rzeczywistych narzędzi.
Standard Qwena używa konstrukcji „Jestem Gama/Delta/Luna”; kryterium dotyczy kanonicznego imienia,
jednego zdania i gotowości, nie pełnej oceny gramatyki.

**Rzeczywisty Core naCPU nadal niezaliczony:**0/1/1filtrowany,158,13s;2/40próby,38nieuruchomionych.
Alfa/Jarvis odpowiada prawdziwie „Nie,jeszcze nie…” po96876ms, Beta/Jarvis kończy`Idle` po60969ms.
Nie podnoszono limitów60/30/120. Osobna sondaHTTP pozwalała zbierać tekst po przekroczeniu warunku
czasowego, oznaczając taki warunek jako niezaliczony;600s to sufit pomiaru, nie nowy próg akceptacji.
Trzy z30sond mają obserwowane przekroczenia odstępu liniiSSE. LinieSSE są przybliżeniem porcji transportu;
nie zastępują werdyktu Core. Wszystkie sondy mają `app_qualified=false`.
CzasyHTTP po rozgrzaniu serwera:3337–98386ms,mediana24233,5ms; nie są pomiaremGPU.

**RTX2060 12GB:** szacunek manifestu dla kontekstu8192 to6369MiB dla LLM (wagi+narzut+KV),
7869MiB z rezerwąSTT1500MiB. To mieści się nominalnie w12GiB, lecz nie jest pomiaremVRAM ani szybkości.
TestCPU nie potwierdza modelu-sędziego, wywołań narzędzi, mikrofonu ani wydajności tej karty.
Qwen nie został wpisany do produkcyjnego katalogu ani ustawiony domyślnie.

[30pełnych żądań i odpowiedzi](artifacts/qwen3-8b-quality-screening.jsonl) ·
[ocena sondy](artifacts/qwen3-8b-quality-review.json) ·
[źródło,piny,Core i metodologia](artifacts/qwen3-8b-probe-provenance.json) ·
[manifest wyłącznie do ręcznego testu](artifacts/qwen3-8b-test-model-entry.json).
