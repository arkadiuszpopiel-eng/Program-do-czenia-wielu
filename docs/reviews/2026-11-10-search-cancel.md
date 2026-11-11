# Stop na granicy odpowiedzi wyszukiwarki — 10.11.2026

Karta [#60](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/60),
worktree `/workspace/wt-search-cancel`, rodzic lokalny `d6d18bbe` z deduplikacją [#59](2026-11-10-search-dedup.md).
Opublikowana baza: `74f0991477300ee7a33281dcc89610e4dcef1c01`.

## Odtworzony błąd

SearchPort może ustawić token Stop w swoim ostatnim poll i natychmiast zwrócić odpowiedź.
Gałąź dostawcy w `tokio::select!` zostaje wtedy wybrana jako gotowa. Bez ponownego sprawdzenia
tokenu narzędzie oddawało `Ok` ze źródłami, mimo że Stop był już zgłoszony. Dostawca zwracający
błąd w tej samej sytuacji oddawał błąd I/O zamiast anulowania. Test odtwarza to deterministycznie:
ustawia token i zwraca wynik bez await, losowania, sleep lub konkurencyjnego obciążenia.

Ponadto wcześniejszy komunikat „Anulowano … (nie wykonano)” mógł sugerować, że zapytanie
nie wyszło do dostawcy. Samo anulowanie oczekiwania tego nie dowodzi.

## Zmiana

Token jest sprawdzany po odpowiedzi dostawcy i zwolnieniu zgody oraz ponownie przed oddaniem
zbudowanego wyniku. Zgłoszony już Stop daje `Cancelled` z pustym payloadem; odrzucone źródła
nie są przekazywane modelowi jako udany wynik. W oczekiwaniu na port ignorujący token nadal
działa niezależna gałąź anulowania. Sukces i błąd bez Stop pozostają dotychczasowe.

Komunikat po rozpoczęciu oczekiwania brzmi: „Anulowano odbiór wyników wyszukiwania.
Zapytanie mogło już dotrzeć do dostawcy.” Nie stwierdza zatrzymania zewnętrznego serwera,
braku egress lub cofnięcia kosztu/efektów zapytania. Anulowanie przed wejściem do narzędzia
zachowuje istniejącą obsługę. Nie zmieniono timeoutów, schematów, polityk, uprawnień lub evals.

## Wyniki

| Kontrola | Wynik |
| --- | --- |
| RED nowego search_cancel | **1 zaliczony / 3 nieudane / 0 ignorowanych**, kod 101 |
| GREEN nowego search_cancel | **4/0/0**, 5 wariantów z kontrolą sukces/błąd |
| Pełne tools-net-contract/impl/fake, all-features | **44/0/0**, kod 0, 11 wyników celów z doctestami |
| Clippy all-targets/all-features | kod 0, bez ostrzeżeń |
| Format i lint | kod 0, bez naruszeń |

Próba wiszącego portu używa Notify do potwierdzenia wejścia przed Stop i wirtualnego zegara
z watchdogiem 1 s; nie jest pomiarem opóźnienia rzeczywistego API. Testy sprawdzają również
wywołanie dostawcy i wydanie zgody na host. Zwolnienie zgody w tej ścieżce nadal następuje
przed oddaniem wyniku lub anulowania. Dotychczasowe kontrakty, testy pobierania, sekretów,
limitów i deduplikacji pozostają zielone.

Po przełączeniu środowiska zniknęły uchwyty do zakończonych procesów Clippy/lint/format.
Ich logi zachowano; do potwierdzenia kodów wyjścia wykonano krótkie ponowienie kontroli,
zapisując wynik każdego polecenia w pliku. Nie ponawiano niezaliczonej próby modelu i nie
zastępowano porażki pojedynczym sukcesem. Pierwszy Clippy zakończył log po 19,47 s;
potwierdzone ponowienie z pamięci podręcznej miało kod 0.

```sh
export CARGO_TARGET_DIR=/workspace/target-nightly
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo +1.94.1 test --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-features -j2
cargo +1.94.1 clippy --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-targets --all-features -j2 -- -D warnings
cargo +1.94 fmt --all -- --check
pnpm lint
```

[Walidacja, komendy, kody i odciski](artifacts/search-cancel-validation.json).
Logi: `/workspace/scratch/search-cancel-reply/`. Nie uruchamiano równolegle rzeczywistego LLM
ani czasowych testów UI. Testy korzystają z atrap SearchPort i harnessu Brokera, bez kluczy
właściciela i bez połączeń z wyszukiwarką.

## Publikacja i dalszy odbiór

Przed publikacją zachować pełny wynik CI 173/Windows 95 poprzedniego heada 74f09914,
bez przerywania go nowym pushem. Poprawki #59/#60 wymagają własnego CI oraz świeżego
niezależnego przeglądu. Nie scalono PR. Odbiór dostawcy WWW, prawdziwego API/hybrydy,
RTX 2060 12 GB, Windows/WebView2 tej poprawki i mikrofonu pozostaje otwarty.
Jakość tekstu #41/#48 nie została naprawiona; [kandydat #58](2026-11-10-branch-tool-facts.md)
pozostaje odrzucony. Obowiązują terminy właściciela: rozwój przed 04:30 UTC, tylko końcowe
testy i raport do 05:00 UTC, potem STOP.
