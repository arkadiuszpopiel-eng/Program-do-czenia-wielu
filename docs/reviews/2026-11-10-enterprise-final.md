# Raport zamknięcia okna Enterprise — 10.11.2026

Stan źródeł: `4c683568582586343c6c1cc26daed309fec2ee62`, gałąź `feat/nightly-modes-jarvis`.
[Draft PR #24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24) pozostaje
niescalony. Raport przygotowano przed końcem okna; nie deklaruje pracy aż do 06:00 ani
ukończonego odbioru Enterprise. Rozwój tylko przed 05:30 Europe/Warsaw, potem testy i raport;
bezwzględny STOP 06:00 (05:00 UTC). Dalszy rozwój wymaga nowego zlecenia właściciela.

## Co przygotowano i sprawdzono

- Pięć trybów: Plan, Ręcznie, Akceptuj edycje, Automatycznie, Pełny dostęp. Jawny zakres
  rozmowy/globalny, Plan bez aktywnej rozmowy i dotychczasowe zgody mają regresje UI/kontraktów.
  Wspólny profil Jarvis jest dostępny dla wbudowanych i własnych person. Alfa, Beta, Gama,
  Delta mają też osobne przełączniki; własna Luna dziedziczy profil wspólny. Indywidualne
  przełączniki własnych person nie zostały dodane. Nie oznacza to zaliczenia jakości modelu.
- Trwała obsada po restarcie (#37), etykiety PL/EN Kreatora (#53), zatwierdzone importy
  testu Brokera (#51), anulowalne odstępy ponowień HTTP504 przy czterech próbach (#52).
- SQLCipher 4.20/SQLite 3.54.1/OpenSSL: naprawa RUSTSEC-2026-0459/CVE-2026-119375;
  sprawdzono starszą szyfrowaną DB i WAL. Wąska zgoda licencyjna dotyczy wyłącznie
  sqlcipher-src420.0.0 i łatki SHA256 `51e27c1036d0d530c125e967268e40e3fc03db334f324815c3cb6cb9cf642456`.
- Router #54 kończy własną trasę po Stop mimo adaptera ignorującego token, zwalnia half-open,
  zachowuje częściowy tekst i nie odtwarza rozpoczętego narzędzia na innym modelu. Testy58/0/0
  obejmują atrapy tras lokalnie→API, API→lokalnie i każdą osobno; nie są odbiorem płatnego API.
- #55 zachowuje pełny zbiór UI, ten sam build i bramkę wymagającą wszystkich trzech części.
  Obecne915 to wszystkie wcześniejsze905 oraz10 dodanych w#57; limity pozostały bez zmian.
- #56/#59/#60/#61: ograniczenie projekcji wyników SearchPort i kontrola URL/sekretów,
  deduplikacja bez zużywania budżetu przez duplikat, Stop przed oddaniem źródeł i sprawdzenie
  jednokrotnie dekodowanych query. To nie jest produkcyjny dostawca wyszukiwania.
- #57 rozróżnia propozycję wymagającą zgody od wykonania w rozmowie, Replay i DTO/IPC.
  Otwarcie terminala nie poświadcza wykonania polecenia; zachowano pozostałe tokeny Undo.
- #62 (`cea4444a`): Stop przed/po nagłówkach, fragmencie i EOF HTTP oraz przed commit pobrania.
  Lokalnie56/0/0; udane zatwierdzenie pliku zachowuje rzeczywisty wynik.
- #63 (`4c683568`): Stop i istniejący deadline kończą oczekiwanie na begin/write magazynu.
  RED2 zaliczone/4 nieudane → GREEN6/0/0; pełne tools-net62/0/0. Rozpoczęte synchroniczne I/O
  może jeszcze wracać w tle, a część znika po powrocie pracownika/Drop. Rozpoczęty commit
  pozostaje obserwowany, aby nie ukryć rzeczywistego pliku.

## Pełne CI dokładnego opublikowanego kodu

[CI176](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42751408643)
**10/10 zadań sukces** i [Windows98](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42751408682)
**4/4 sukces**, oba dla `4c683568`. Kolejność liczb: zaliczone/nieudane/ignorowane.

| Kontrola | Dokładny wynik |
| --- | --- |
| Rust Linux, all-features | **3550/0/26**,1132 wyników celów |
| Rust Windows, all-features | **3576/0/47**,1132 wyników celów |
| Chromium headless Linux | **915/915**,305+305+305;8,4/8,5/9,7 min |
| UI Vitest | **573/573**,103 pliki |
| Svelte UI i ui-kit | 0 błędów,0 ostrzeżeń |
| Windows/Tauri/WebView2 przez CDP | **63/0/5**,2,7 min |
| Clippy, format, schematy, dokumentacja, cargo-deny i build | właściwe zadania/kroki sukces |

[Wyniki, identyfikatory zadań i SHA256 surowych logów](artifacts/ci176-final-results.json).
Pełne logi zachowane w `/workspace/scratch/enterprise-final-2026-11-10/ci-logs/`.
CI176 trwało od03:44:46 do04:24:21 UTC: **39 min35 s**. Poprzednich pełnych przebiegów
nie przerwano kolejnym pushem. WebView2 jest rzeczywistą aplikacją CI Windows, ale nie
odbiera sprzętu właściciela, mikrofonu, GPU ani rzeczywistej usługi wyszukiwania.

Końcowa kontrola opublikowanego źródła: tools-net **62/0/0**,13 wyników celów, kod0,
04:29:52–04:29:54 UTC. Lint raportu kod0, generator/weryfikacja102 stron kod0.
[Komendy, czasy i hashe końcowych kontroli](artifacts/enterprise-final-checks.json).

## Kandydat #64 zachowany poza gałęzią PR

[Snapshot d4f50f2a](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/commit/d4f50f2ad8d6332ffec84616c844fef8756ceee5)
(tree `d8749b3ac0b0bedffbc6b388b54597158e3781dc`, lokalny commit `0b67c217`) dodaje
sprawdzenie istniejącego deadline przed/po gotowym wyniku HTTP/SearchPort. Bez tej poprawki
`timeout_at` przy gotowej future dopuszcza spóźniony sukces lub błąd I/O zamiast Timeout.

Rzeczywisty RED **0/6/0**, kod101; kandydat pełne tools-net **68/0/0**,14 wyników celów,
Clippy/fmt/lint/rozmiar i generator/weryfikacja102 stron kod0. Testy używają bramek portów
oraz wirtualnego zegara, nie zwiększają limitów i nie odbierają opóźnień rzeczywistego API.
[Dokładny raport kandydata](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/blob/d4f50f2ad8d6332ffec84616c844fef8756ceee5/docs/reviews/2026-11-10-ready-deadline.md).

**Nie zintegrowano #64 z PR i nie wykonano jego pełnego CI.** Gdy poprzednik ukończył CI,
pozostało35 min39 s do końca okna, mniej niż zmierzone39 min35 s. Zachowano kod, łatkę,
wyniki i hashe zamiast rozpoczynać przebieg wykraczający poza termin. Zielone CI176 nie
jest przypisywane #64. Wymaga nowego okna, niezależnego przeglądu i własnego pełnego CI.

## Niezaliczone i niewykonane próby

**#41/#48 pozostają otwarte.** Rzeczywisty Core+Bielik1.5B w#58: baza50/50 fikcyjnych
potwierdzeń działań i kandydat50/50, mimo0 narzędzi i poprawnych liczników0. Transport
bez błędów i ucięć nie zalicza treści. Zmianę kontekstu odrzucono i wycofano; zachowano
100 żądań/odpowiedzi, źródła i hashe. [Raport#58](2026-11-10-branch-tool-facts.md).

Wcześniejsza baza050f: imię40/50, jedno zdanie10/50. Arytmetyka:22/50 odpowiedzi,
5 ucięć,28 niewykonanych. Bielik4.5B nie zaliczył budżetów CPU. Qwen3-8B Q4 miał30/30
poprawnych treści w pojedynczej sondzie, ale pełny CoreCPU przerwano po2/40 próbach:
Beta Idle60969 ms,38 niewykonanych. Brak dowodu zgodności czasowej na RTX2060 **12GB**;
nie zmieniono domyślnego modelu ani progów. [Próby modeli](2026-11-09-jarvis-model-repair.md).

Produkcja nadal ma port SearchPort bez wybranego dostawcy. Nie wykonano odbioru rzeczywistej
wyszukiwarki, trafności PL/EN, aktualności i cytowań. Wybór Brave Search API/Tavily/własny
SearXNG oczekuje właściciela; nie użyto przypadkowej usługi ani kluczy z plików/CLI.
Rzeczywiste płatne API/hybryda, GPU12GB i prawdziwy mikrofon pozostają niewykonane.
Nie uruchamiano nowych mostów CLI, subagentów, zmian polityk lub zamrożonych evals.

## Plan po nowym zleceniu

1. Niezależny przegląd PR#24 i kandydata#64; dopiero potem kontrolowana integracja#64
   i własne pełne CI. Bez merge na podstawie samego CI poprzednika.
2. #41/#48: powtarzalna jakość wszystkich person w obu profilach przy istniejących budżetach;
   osobny pomiar8B na docelowym RTX2060 12GB, bez ukrywania nietrafnych odpowiedzi.
3. Wybrać uprawnionego dostawcę WWW; adapter, błędy/anulowanie, źródła/cytowania oraz ocena
   trafności/aktualności PL/EN. Sekret wyłącznie przez zatwierdzoną konfigurację, nigdy czat.
4. Odbiór rzeczywistej aplikacji offline/lokalnie,API,hybryda: awarie,429/5xx,restart,
   zgody i potwierdzone efekty narzędzi. Osobno GPU,mikrofon i komputer właściciela.

Bieżący stan procesów, dzierżawy, komendy/kody wyjścia, odciski i końcowe znaczniki czasu
zapisuje `/workspace/scratch/nightly-status.json`. Testy z atrapami Linux, model CPU,
Chromium i Windows/WebView2 są osobnymi dowodami. Nie deklarujemy ciągłej pracy ani
usunięcia wszystkich usterek. Po końcowym zapisie prace ustają do odwołania.
