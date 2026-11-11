# Raport napraw Alfy i Jarvisa — 9.11.2026

**Naprawy pięciu usterek kodu są opublikowane i mają zielone pełne CI. Usterki jakości modelu #41/#48 pozostają otwarte. Zlecona próba Qwen3-8B została wykonana: 30/30 poprawnych odpowiedzi w sondzie jakości, ale pełny Core na CPU niezaliczony. Sprzęt docelowy: RTX 2060 12GB.**

Repozytorium: `arkadiuszpopiel-eng/Program-do-czenia-wielu`, gałąź `feat/nightly-modes-jarvis`.
[Draft PR #24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24).
Korekta właściciela z06:53UTC zastąpiła dawny termin08:00 czasu polskiego: wyłącznie naprawy obecnych usterek,
bez nowych funkcji. Nie scalono PR i nie wydano0.0.85. Automatyzacja nocna pozostaje wyłączona.

## Naprawy kodu

| Usterka | Zmiana i wynik |
| --- | --- |
| #53 — surowe etykiety Kreatora | Uzupełnione PL/EN system/vision i kolorów; regresje Chromium i Windows/WebView2, obejrzany obraz |
| #37 — utrata obsady po restarcie | Append-only zapis w szyfrowanej bazie i odtworzenie ról, walidacja katalogu i sprzątanie nieużywanych połączeń;76 testów modułów i17 integracji, Luna rzeczywiście zachowuje rolę po restarcie |
| #51 — LinuxClippy Brokera | Wyłącznie importy KillReason/KillSwitch pod cfg(windows), po zgodzie właściciela „ok”;4/4 i pełny Clippy bez ostrzeżeń |
| #52 — chwilowe HTTP504 | Anulowalne przerwy250/500/1000ms w dotychczasowych ponowieniach;82/0/1, bez zwiększania czterech żądań, timeoutów lub zmiany pinów; dostępność obcego serwera nadal może zawieść |
| RUSTSEC-2026-0459 / CVE-2026-119375 | Rzeczywisty SQLite3.54.1 / SQLCipher4.20 / OpenSSL; regresja starej szyfrowanej bazy i aktywnegoWAL, zachowane szyfrowanie i format klucza |
| #41/#48 — odpowiedzi Bielika | **Nierozwiązane:** błędne imiona Standard Bety/Gamy, niezachowywanie liczby zdań i nieprawdziwe deklaracje o wykonanych czynnościach;4.5BQ8 na tym CPU także nie przechodzi wszystkich limitów |

Pięć trybów pracy (Plan, Ręcznie, Akceptuj edycje, Automatycznie, Pełny dostęp) ma regresje zakresu rozmowa/globalnie,
Planu bez rozmowy i kłódki. Testy modeli poniżej dotyczą dwóch profili współpracy Standard/Jarvis;
nie są odbiorem wszystkich działań w pięciu trybach na sprzęcie właściciela. Własne persony, w tym Luna,
dziedziczą wspólny profil Jarvisa; indywidualne przełączniki własnych person pozostają w zamrożonym backlogu.

## Pełne CI kodu produkcyjnego050f86c7

Pełny commit: `050f86c725bb474ae0a06d6dccecd044c86dd862`, drzewo `487561306c262805b1cc2f36336409335374a901`.
[CI168 — sześć zadań zielone](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42662594816).
[Windows90 — cztery zadania zielone](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42662594491).

| Zakres | Dokładny wynik |
| --- | --- |
| Rust Linux | 3506 zaliczone /0 nieudanych /26 ignorowanych;1126 oryginalnych wyników celów |
| Rust Windows | 3532/0/47;1126 oryginalnych wyników celów |
| Clippy obu platform | Bez ostrzeżeń |
| Jednostkowe UI | 573/573,103 pliki |
| Chromium E2E i axe | 905/905,29,3min |
| Svelte | 0 błędów /0 ostrzeżeń |
| Tauri | 5 testów produkcyjnych i4 testowe zaliczone |
| cargo-deny | Aktualna polityka: advisories/bans/licenses/sources zaliczone |
| Pozostałe kontrole | Lint, build, budżet bundla, Storybook, generatory i dokumentacja zaliczone |
| Windows/WebView2/CDP | 63 zaliczone /0 nieudanych /5 pominiętych,2,8min; Jarvis3,1s, kłódkaDziecko3,7s |
| Silniki CPU | 1 zaliczony /0 nieudanych /2 filtrowane,846,27s |
| Syntetyczny głos | PiperTTFB457ms, audio2185ms;WhisperWER0,25516ms,RTF11,68 |

Obie nowe regresje SQLCipher przeszły na Linuksie i Windows. CUDA nie wystartowało na runnerze;
zaliczony test silników używa fallbackCPU. Pięć pominiętych testów WebView2 pozostaje pominiętych:
specjalny profil poziomu, logowanie CLI, natywny eksport i dwa skróty głosowe w widocznym pulpicie.
WebView2 to debug/dev na runnerze, emulowany ekran i sędzia w procesie.
**Nie wykonano odbioru komputera właściciela, RTX2060/12GB, oddzielnego produkcyjnego Brokera ani prawdziwego mikrofonu.**

## SQLCipher i zakres zgody właściciela

CI167 wykryło rzeczywistą podatność SQLite używanego przez SQLCipher. Nie ignorowano advisory ani nie ponawiano
samego skanu, żeby ukryć alarm. Zwykła aktualizacja rusqlite/sys zostawiałaby podatny SQLCipher419.
Zastosowano rusqlite0.41.1 i kontrolowaną kopię libsqlite3-sys0.39.1 z dokładnym sqlcipher-src420.0.0;
oba lockfile zaktualizowane. Adapter odtwarzany z oficjalnego archiwum z SHA256,29 plików kontrolowanych
w lint; bez nowego źródłaGit i bez wyjątku rozmiaru. ADR0030 pozostaje propozycją do niezależnego przeglądu.

Właściciel odpowiedział „Zatwierdzam wyłącznie tę łatkę”. Zastosowano **tylko blessing/WTFPL dla sqlcipher-src@=420.0.0**;
nie rozszerzono globalnej listy licencji, ignorowanych podatności, źródeł ani uprawnień.
SHA256 zatwierdzonej łatki: `51e27c1036d0d530c125e967268e40e3fc03db334f324815c3cb6cb9cf642456`.
Ta zgoda nie obejmuje innych polityk lub całego PR. Lokalnie33/0/1 bazy,73/0/3 odbiorców,15/0 Core,
pełny workspace/all-targets Clippy0, aktualny cargo-deny4/4. Pełna lokalna próba Core nie doszła do wykonania
przez brak miejsca podczas linkowania na32GB Cloud (101); nie jest zaliczeniem. Po usunięciu wyłącznie starych
binariów cache wybrane testy przeszły; pełny kod potwierdza odrębne CI168 powyżej.
[Pełny raport SQLCipher](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/blob/feat/nightly-modes-jarvis/docs/reviews/2026-11-09-ci167-sqlite.md).

## Rzeczywiste próby modeli i niezaliczone przypadki

| Model i procedura | Wynik i ograniczenia |
| --- | --- |
| Bielik1.5B Q8_0, rzeczywisty Core, 100 odpowiedzi | 0 błędów transportu i obcięć; to nie jest zaliczenie jakości. Imię40/50, jedno zdanie10/50. Pytanie o wykonanie działań: **50/50 fałszywych potwierdzeń**, mimo0 narzędzi |
| Bielik1.5B, dodatkowa arytmetyka | 22/50 planowanych wierszy; oba przebiegi Rust niezaliczone,5 obcięć łącznie, pusty tekst,28 prób nieuruchomionych |
| Bielik4.5B Q8_0, produkcyjny Core | Pierwsza Alfa/Standard: Idle72332ms, test0/1/1 filtrowany;39/40 prób nieuruchomionych |
| Próby skrócenia pustego kontekstu4.5B | Kandydat1:40/40 transport, ale usuwał tytuł rozmowy — odrzucony. Kandydat2 zachowuje tytuł,52/0/0, ale zimny JarvisIdle65862ms — także nieprzyjęty. Kod produkcyjny przywrócony |
| Qwen3-8B Q4_K_M, **sonda jakości** | **30/30**:10/10 prawdziwych zaprzeczeń wykonania działań,10/10 kanoniczne imię+jedno zdanie+gotowość,10/10 sama poprawna liczba. Alfa,Beta,Gama,Delta,Luna × Standard/Jarvis ×3 pytania, po1 powtórzeniu |
| Qwen3-8B, **rzeczywisty Core** | **0 zaliczonych /1 niezaliczony /1 filtrowany**,158,13s;2/40 próby,38 nieuruchomionych. Alfa/Jarvis prawdziwe „Nie…”96876ms; Beta/JarvisIdle60969ms |

Sonda Qwena to osobny pomiar HTTP, nie pełna integracja aplikacji:3/30 obserwowane przekroczenia bramek czasu
(linieSSE, przybliżenie porcji transportu), każda odpowiedź `app_qualified=false`.600s było wyłącznie sufitem zbierania
wyniku po oznaczeniu porażki, nie nowym progiem akceptacji. Nie zwiększono produkcyjnych60/30/120s.
Sonda używa oryginalnych instrukcji050f,96 tokenów,temperatury0,oficjalnego `enable_thinking=false`.
Brak obcięć, wywołań narzędzi i zwróconej treści rozumowania. Standard używa też formy „Jestem Gama/Delta/Luna”;
kanoniczne imię nie oznacza zaliczenia pełnej gramatyki. Czasy CPU HTTP3337–98386ms,mediana24233,5ms.

Oficjalny model: Qwen/Qwen3-8B-GGUF, rewizja `7c41481f57cb95916b40956ab2f0b139b296d974`,
5027783488 bajtów, zweryfikowany SHA256 `d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785`.
Bielik1.5B SHA256 `e206ebbd9ee00cc8f14af5a096ad3f193a59608c63905316e8dc680c96655a60`;
Bielik4.5B SHA256 `562f2291de257890adf2b4a914da8b194affe6a7a838a6b7ef3d342f306c1b7f`.
Qwen jest w Q4, Bieliki w Q8; nie jest to porównanie przy identycznej kwantyzacji.

**Wniosek dla RTX2060 12GB:** Qwen3-8B jest obiecującym kandydatem do testu na tej karcie.
Szacunek manifestu dla8192 kontekstu:6369MiB LLM z KV;7869MiB (~7,7GiB) z rezerwąSTT1500MiB.
To szacunek, nie zmierzony VRAM. Cloud nie ma GPU. Nie zmierzono szybkości, współpracy ze STT/TTS,
rzeczywistych narzędzi ani modelu-sędziego na tej karcie. Qwen nie zastąpił modelu domyślnego.

[Pełne wyniki i niezaliczone próby modeli](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/blob/feat/nightly-modes-jarvis/docs/reviews/2026-11-09-jarvis-model-repair.md),
łącznie z odrzuconą hipotezą46 (1.5B25/38 wyników diagnostycznych;4.5B34 odpowiedzi/35 prób/38 planowanych,
przerwane żądanie~190,6s, wada starej procedury deadline) oraz surowymi wejściami i odpowiedziami.
Żaden odrzucony prompt lub skrót kontekstu nie został przyjęty do produkcji.

Przy badaniach w Cloud nie uruchamiano równolegle modeli i ciężkich testów Cargo/UI.
Pomiary CPU nie pozwalają określić szybkości GPU ani zaakceptować działania na RTX2060.

## Zmiany testów i zapis

Publikowana diagnostyka zapisuje końcowe pytanie faktycznego dostawcy, pozwala użyć innego publicznego pytania
przez `ALFA_LIVE_QUERY` i zacząć macierz wbudowanych person od zimnego Jarvisa (`ALFA_LIVE_JARVIS_FIRST=1`).
Domyślne pytanie, pięć powtórzeń,96 tokenów,temperatura0, zdolności modeli i limity60/30/120s pozostają dotychczasowe.
Luna ma5 prób przed restartem i5 po, bez ponownego przypisywania roli. Ustawienie kolejności nie zmienia tej kontroli.
Nie dopisywano gotowych odpowiedzi i nie przepisywano tekstu modelu, żeby uzyskać zaliczenie.

Lokalna regresja kontekstu i Core52/0/0 przed przywróceniem produkcyjnego kontekstu; końcowa diagnostyka dodatkowego modelu:Clippy0, kompilacja0,format i rozmiary0,odrzucenie duplikatu wbudowanego ID zgodne z oczekiwaniem. Lint i generatory dokumentacji bez naruszeń.
Kod produkcyjny pozostaje równoważny zielonemu050f86c7. Wszystkie niesprawdzone zmiany promptów/kontekstu wycofano;
łatki i surowe odpowiedzi zachowano do przeglądu. Aktualizowane są raport, roadmapa,plan54 i generowana dokumentacja.

Raport, surowe wyniki i diagnostyka testowa trafiają na istniejącą gałąź w draft PR#24. Kod produkcyjny pozostaje050f86c7; końcowy commit testów i dokumentacji jest zapisany w checkpoint i historii PR. Pełne CI168 opisane wyżej dotyczy050f86c7; nowy commit diagnostyczny wymaga osobnego CI przed ewentualnym scaleniem.

Dane lokalne: `/workspace/scratch/nightly-status.json`, `/workspace/scratch/final-repair-report/`,
`/workspace/scratch/jarvis-final-core-probes/`, `/workspace/scratch/chat-empty-context/`.
Komendy i odciski binariów są w `source.json` i `status.json` odpowiednich przebiegów.

Polecenia ostatniej regresji:

```sh
export PATH=/home/agent/.cargo/bin:$PATH
export CARGO_TARGET_DIR=/workspace/target-nightly
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo +1.94.1 test --locked -p app-chat -p app-core --lib --test project_prompt_context --test jarvis --test jarvis_custom -j2
cargo +1.94.1 clippy --locked -p app-chat -p app-core --lib --test project_prompt_context --test jarvis --test jarvis_custom --test jarvis_live -j2 -- -D warnings
```

Próby modelu wykonuje ręczne `jarvis_live` z rzeczywistym serwerem b11559 i oficjalnymi GGUF (BielikQ8_0,QwenQ4_K_M),
których pełne SHA256 zweryfikowano. Powtórzenie zapisuje wszystkie wyniki, także błędy i obcięcia.
Piper→Whisper na syntetycznym nagraniu jest osobnym badaniem od dialogu z prawdziwym mikrofonem.

## Dalsze kroki i stan zadania

#41/#48 pozostają otwarte. Potrzebny jest osobny odbiór jakości odpowiedzi i konfiguracji modelu;
nie ma podstaw, by zaakceptować wszystkie persony na4.5BQ8 w tym CloudCPU lub na niebadanym RTX2060.
Przed scaleniem nadal wymagany świeży niezależny przegląd; zielone testy nie zastępują tego przeglądu.
Nowe funkcje pozostają zamrożone, automatyzacja wyłączona. Zapis raportu nie oznacza usunięcia usterek modelu.
W chwili zakończenia pomiarów nie działa żaden proces naszych prób modelowych ani Cargo/UI. Pozostałe systemowe procesy środowiska pozostają nienaruszone; zombie nie są trwającym przebiegiem. Po zapisaniu raportu i publikacji kończę tę sesję prac bez uruchamiania nowych ulepszeń. Otwarte kwestie wymagają odbioru modelu na docelowym GPU i niezależnego przeglądu.
