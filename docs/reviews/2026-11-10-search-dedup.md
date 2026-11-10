# Deduplikacja źródeł wyszukiwania — 10.11.2026

Karta [#59](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/59),
baza `74f0991477300ee7a33281dcc89610e4dcef1c01`, worktree `/workspace/wt-search-dedup`.
Kontynuacja [planu 55](../plan/55-enterprise-lokalnie-chmura-i-wyszukiwanie.md).

## Problem i zachowanie po poprawce

Przy limicie dwóch wyników dostawca zwracający `[A,A,B]` zajmował oba miejsca tym samym
źródłem i ukrywał B. Długi tytuł powtórzonego trafienia mógł też zużyć budżet znaków B.
Test przed zmianą potwierdził oba przypadki oraz duplikaty różniące się fragmentem URL.

Projekcja zachowuje pierwszy wynik i kolejność dostawcy. Klucz duplikatu pochodzi z już
sprawdzonego `Target::as_str()`: istniejąca normalizacja hosta i domyślnego portu, bez
fragmentu. Zwracane tytuł, opis i oryginalny URL pierwszego trafienia pozostają jego danymi,
z dotychczasową redakcją pól tekstowych. Nie łączymy treści różnych trafień. Różne ścieżki,
wielkość liter w ścieżce, query i porty pozostają osobne. Nie usuwamy parametrów śledzących
ani nie zgadujemy tożsamości treści stron. Fragmenty tego samego celu HTTP nie zajmują
osobnych miejsc; pierwszy oryginalny fragment nadal jest w zwróconym adresie.

Duplikat nie zużywa miejsca lub znaków, a pominięcie jest jawne w tekście wyniku. Odrzucenie
sekretów i niedozwolonych adresów następuje przed zapamiętaniem klucza, więc wadliwy wpis
nie blokuje późniejszego poprawnego źródła. Zbiór kluczy obejmuje tylko zatrzymane wyniki:
jest ograniczony istniejącym `max_results` (1–20). Nie wykonujemy dodatkowych zapytań,
by uzupełnić listę; mniej unikatowych trafień dostawcy może dać mniej niż zadany limit.

Nie zmieniono kontroli HTTPS, DNS, Brokera, polityk, progów evals, zależności i schematów.
SearchPort nadal zwraca już zaalokowany wektor; to nie jest limit dekodera przyszłego adaptera.

## Sprawdzenie

| Kontrola | Wynik |
| --- | --- |
| RED search_results | 7 zaliczonych / **4 nieudane** / 0 ignorowanych, kod 101 |
| GREEN search_results | **11/0/0**, w tym 5 nowych testów |
| Pełne tools-net-contract/impl/fake, all-features | **40/0/0**, kod 0, 10 wyników celów z doctestami |
| Clippy całej trójki, all-targets/all-features | kod 0, bez ostrzeżeń, 22,82 s |
| Format i lint | kod 0, bez naruszeń |

Nowe regresje sprawdzają limit miejsc, duży tytuł duplikatu, zachowanie pierwszego rankingu
i oryginalnego fragmentu, host/port/fragment oraz odrębność ścieżek/query/portów. Próba z
syntetycznym sekretem w fragmencie potwierdza, że filtr działa przed deduplikacją.
Dotychczasowe testy sekretów, budżetów Unicode, odrzuconych źródeł, pobierania, zgód,
anulowania i kontraktów również pozostały zielone.

```sh
export CARGO_TARGET_DIR=/workspace/target-nightly
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo +1.94.1 test --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-features -j2
cargo +1.94.1 clippy --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-targets --all-features -j2 -- -D warnings
cargo +1.94 fmt --all -- --check
pnpm lint
```

Zapis maszynowy i odciski: [walidacja](artifacts/search-dedup-validation.json).
Logi: `/workspace/scratch/search-result-dedup/`. Nie uruchamiano równolegle ciężkiego modelu
lub czasowych testów UI. Test dostawcy jest deterministyczną atrapą; nie wykonuje wyszukiwania
internetowego ani zapytań do przypadkowych usług.

## Publikacja i granice

Przed publikacją wymagane zakończenie pełnego CI 173/Windows 95 poprzedniego heada 74f09914;
nie przerywać go kolejnym pushem. Nowa poprawka wymaga własnego CI i świeżego niezależnego
przeglądu przed scaleniem. Lokalny GREEN nie jest odbiorem Windows/WebView2 ani GPU.

To poprawka projekcji. Produkcyjny dostawca WWW, ocena trafności PL/EN, źródła i cytowania,
API/hybryda, RTX 2060 12 GB i prawdziwy mikrofon pozostają do odbioru. #41/#48 są nadal otwarte;
odrzucona próba kontekstu #58 nie została włączona do aplikacji. Okno rozwoju kończy się
o 04:30 UTC; tylko końcowe testy/raport do 05:00 UTC, potem STOP według właściciela.
