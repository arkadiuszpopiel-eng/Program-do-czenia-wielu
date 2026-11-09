# Naprawy obecnych usterek i próby Bielika, 9.11.2026

Gałąź: `feat/nightly-modes-jarvis`, draft [PR #24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24).
Korekta właściciela z 06:53 UTC zastąpiła dawny termin: tylko obecne usterki, bez nowych funkcji.
Po rozliczeniu wyników i zapisie koniec prac do odwołania. Automatyzacja nocna pozostaje wyłączona.
To raport autorski, nie wymagany niezależny przegląd.

**Cztery usterki mają sprawdzone naprawy. #41/#48 nadal nie są usunięte.**
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
przepisywaniem tekstu modelu ani fikcyjnym meldunkiem o wykonaniu. Produkcyjne źródła wróciły
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

