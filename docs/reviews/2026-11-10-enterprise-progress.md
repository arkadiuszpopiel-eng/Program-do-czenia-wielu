# Raport postępu prac Enterprise — 10.11.2026

Stan po przygotowaniu #63, przed jego pełnym CI; to nie jest raport końcowy o 06:00.
Właściciel wyznaczył rozwój przed **05:30 Europe/Warsaw (04:30 UTC)**, potem tylko końcowe
testy i raport, zakończenie **06:00 (05:00 UTC)**. Późniejsza korekta ma pierwszeństwo.

[Draft PR #24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24),
gałąź `feat/nightly-modes-jarvis`. Bez merge lub wydania; jakość Enterprise nie jest odebrana.

## Przygotowane naprawy

- Trwała szyfrowana obsada i restart (#37), etykiety PL/EN Kreatora (#53), zatwierdzone
  importy testu Brokera (#51), anulowalne odstępy po HTTP504 przy czterech próbach (#52).
- SQLCipher 4.20/SQLite 3.54.1/OpenSSL usuwa RUSTSEC-2026-0459/CVE-2026-119375. Testowano
  starszą szyfrowaną DB i WAL. Właściciel zatwierdził wyłącznie blessing/WTFPL dla dokładnego
  sqlcipher-src420.0.0: SHA256 51e27c1036d0d530c125e967268e40e3fc03db334f324815c3cb6cb9cf642456.
- Router kończy własną trasę po Stop mimo adaptera ignorującego token, zwalnia half-open,
  zachowuje częściową odpowiedź i nie zaczyna zapasu po rozpoczęciu narzędzia (#54).
- Pełne E2E działa na trzech runnerach z tym samym buildem i wymagającą wszystkich bramką,
  bez zwiększania limitów (#55). Obecne 915 obejmuje wszystkie wcześniejsze 905 oraz 10 z #57.
- Projekcja SearchPort ma kontrolę URL/sekretów i budżet pól JSON (#56). Propozycje zachowują
  needs_confirmation w rozmowie/Replay/DTO; otwarcie terminala nie wykonuje polecenia (#57).
- #59/#60/#61 w commitach `1495f724`, `102a5da2`, `5fe2edd7`: deduplikacja, Stop przy odpowiedzi
  wyszukiwarki i kontrola dekodowanych query. Lokalnie 47/0/0, 12 nowych testów.
- #62 w opublikowanym `cea4444a`: Stop przy nagłówkach/fragmencie/EOF HTTP i przed commit pobrania.
  Lokalnie **56/0/0**, 9 nowych testów. Udany commit zachowuje prawdziwy wynik pliku.

- #63: Stop i dotychczasowy deadline podczas begin/write magazynu; RED2/4 → GREEN62/0/0.
  Rozpoczęte synchroniczne I/O może wracać w tle; rozpoczęty commit zachowuje wynik.

Szczegóły i dowody: [#59](2026-11-10-search-dedup.md), [#60](2026-11-10-search-cancel.md),
[#61](2026-11-10-search-encoded-params.md), [#62](2026-11-10-http-cancel.md),
[#63](2026-11-10-download-wait.md), [Router](2026-11-09-enterprise-router.md), [CI](2026-11-09-ci-ui-shards.md),
[SQLCipher](2026-11-09-ci167-sqlite.md), [propozycje](2026-11-10-tool-confirmation.md).

## Pełne CI dokładnej bazy

Kod cea4444a ma zielone **CI175 10/10 i Windows97 4/4**. Liczby: zaliczone/nieudane/ignorowane.

| Dowód | Wynik |
| --- | --- |
| Rust Linux, all-features | 3544/0/26, 1131 wyników celów |
| Rust Windows, all-features | 3570/0/47, 1131 wyników celów |
| Chromium | 915/915, 305/305/305, 9,8/9,9/8,1 min |
| Windows/WebView2 | 63/0/5, 2,6 min |
| Clippy | bez ostrzeżeń obu platform |
| UI, schematy, zależności i cargo-deny | zadania zakończone sukcesem |

[CI175](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42746088767) ·
[Windows97](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42746088440) ·
[wyniki](artifacts/ci175-disk-baseline.json).
Nie anulowano poprzednich pełnych przebiegów pushem. Nowy #63 wymaga własnego CI;
nie przypisujemy mu sukcesu bazy. Niezależny przegląd i kryteria sprzętowe pozostają otwarte.

## Niezaliczone próby modeli

#41/#48 są **nierozwiązane**. Prawdziwy Core + Bielik 1.5B dał **50/50 fikcyjnych potwierdzeń**
działań w bazie i tyle samo w kandydacie #58. Obie serie bez błędu transportu i ucięć, 0 narzędzi
w każdym żądaniu i poprawne liczniki 0 u kandydata. Zmianę kontekstu wycofano. Zachowano 100
żądań/odpowiedzi, źródła, binaria i hashe. Zielony transport nie zalicza semantyki.
[Pełny raport](2026-11-10-branch-tool-facts.md).

Wcześniejsza baza 050f: imię 40/50, jedno zdanie 10/50; arytmetyka niezaliczona: 22/50 odpowiedzi, 5 ucięć,
28 niewykonanych. Bielik 4.5B nie zaliczał limitów CPU. Qwen3-8B Q4 miał 30/30 poprawnych treści
w pojedynczej sondzie, ale pełny Core CPU niezaliczony po 2/40 próbach: Beta Idle 60969 ms,
38 niewykonanych. Nie zmieniono modelu domyślnego lub limitów. Szacunek VRAM nie jest
pomiarem RTX 2060 12 GB. [Modele i niepowodzenia](2026-11-09-jarvis-model-repair.md).

## Dalsze kroki

1. Zebrać pełny wynik nowego CI #63 i rozliczyć porażki; zachować go przed kolejnym pushem.
   Wymagany świeży niezależny przegląd przed scaleniem.
2. Właściciel nie wybrał dostawcy WWW. Oczekują Brave Search API, Tavily API lub własny
   SearXNG; milczenie nie jest wyborem. Klucza nie podawać w czacie.
3. Następnie prawdziwy adapter, ograniczony dekoder, anulowanie/błędy, źródła/cytowania
   i trafność PL/EN. SearchPort/net_fetch nie są wyszukiwarką światowej jakości.
4. Odbiór rzeczywistych API/lokalnie/hybrydy, GPU **RTX 2060 12 GB**, mikrofonu i komputera
   właściciela. Atrapy Linux, model CPU i CI Windows/WebView2 są osobnymi dowodami.
5. #41/#48 muszą przejść rzeczywiste próby przy dotychczasowych budżetach. Nie poprawiać
   odpowiedzi modelu po fakcie i nie zmieniać zamrożonych progów lub polityk.

Nie uruchamiano mostów CLI lub nieautoryzowanych subagentów ani nie odczytywano tokenów CLI.
Nowe poprawki nie dotykają chronionych modułów. Nie deklarujemy nieprzerwanej pracy.
Końcowy raport o 06:00 ma podać ostatni dokładny head/CI, niezaliczone i niewykonane próby,
stan własnych procesów i zwolnienie dzierżawy. Potem praca ustaje do odwołania.
