# Kodowane parametry źródeł wyszukiwania — 10.11.2026

Karta [#61](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/61),
worktree `/workspace/wt-search-encoded`, lokalny rodzic `38306b39` z poprawką Stop [#60](2026-11-10-search-cancel.md).
Opublikowana baza to `74f0991477300ee7a33281dcc89610e4dcef1c01`.

## Problem i poprawka

Źródło `https://docs.example/a?%74%6f%6b%65%6e=tajny123` zawiera zakodowaną nazwę `token`.
Dotychczasowa kontrola tekstu URL nie rozpoznawała tej reprezentacji i przekazywała adres
z syntetycznym sekretem do tekstu oraz JSON. Podobnie zakodowany rozpoznawany prefiks
w wartości `ref=%73%6b%2DABCDEFGHIJKLMNOPQRSTUV` omijał kontrolę. Oba przypadki zostały
odtworzone testem rzeczywistego wywołania narzędzia nad atrapą SearchPort.

Projekcja sprawdza teraz również klucze i wartości z `Target::url().query_pairs()` po
standardowym, jednokrotnym dekodowaniu parametrów HTTP. Stosuje ten sam istniejący
`url_carries_secret`, bez nowych wzorców i bez zmiany globalnego detektora lub polityk.
Wykryty adres jest pomijany przed deduplikacją; pozostałe URL pozostają w oryginalnej postaci.
Nie redagujemy sekretu do fikcyjnego źródła. Nadal działa filtrowanie niepoprawnych adresów,
kontrola surowego URL, limit wyników, budżet znaków, taint i Broker.

Nie dodano zależności, nie zmieniono lockfile, schematów, uprawnień, progów lub timeoutów.
Zakres nowej kontroli to parametry query po jednym dekodowaniu. Nie deklarujemy pełnego
wykrywania wszystkich sekretów, wielokrotnych kodowań lub kodowanych danych innych części URL.
Przyszły adapter nadal wymaga własnych limitów odpowiedzi i kontroli sieciowej.

## Wyniki

| Kontrola | Wynik |
| --- | --- |
| RED search_results | **12 zaliczonych / 2 nieudane / 0 ignorowanych**, kod 101 |
| GREEN search_results | **14/0/0**, 3 nowe testy, 7 wariantów danych |
| Pełne tools-net-contract/impl/fake, all-features | **47/0/0**, kod 0, 11 wyników celów z doctestami |
| Clippy all-targets/all-features | kod 0, bez ostrzeżeń, 16,95 s |
| Format i lint | potwierdzone kodami 0, bez naruszeń |

Nowe dane obejmują zakodowane nazwy token/password/api_key, rozpoznawany prefiks w wartości
oraz trzy niesekretne query, w tym Unicode, spacje i zakodowane `=`. Ostatnia kontrola wymaga
zachowania oryginalnego URL; poprawka nie zmienia poprawnych źródeł. Dotychczasowe regresje
deduplikacji #59 i Stop #60 również przechodzą. Łącznie te trzy przyrosty dodają 12 testów
do poprzednich 35; nie zliczamy ponowień jako nowych testów.

```sh
export CARGO_TARGET_DIR=/workspace/target-nightly
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo +1.94.1 test --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-features -j2
cargo +1.94.1 clippy --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-targets --all-features -j2 -- -D warnings
cargo +1.94 fmt --all -- --check
pnpm lint
```

[Wyniki, komendy, kody wyjścia i odciski](artifacts/search-encoded-validation.json).
Logi: `/workspace/scratch/search-encoded-params/`. Wszystkie sekrety są syntetyczne,
bez odczytu tokenów właściciela lub zapytań do zewnętrznych usług. To atrapa dostawcy
i harness Brokera na Linux Cloud; nie jest to żywa wyszukiwarka ani odbiór sprzętowy.

## Pełna baza przed publikacją

CI 173 dla 74f09914 zakończyło wszystkie 10 zadań sukcesem; Windows 95 wszystkie 4.
Linux 3523/0/26, Windows 3549/0/47, po 1129 wyników celów; Chromium 915/915 w częściach
305/305/305 (10,0/8,7/10,8 min), Windows/WebView2 63/0/5 w 2,7 min. Zachowano pełny wynik
przed aktualizacją gałęzi, bez anulowania przebiegu. [Zapis z identyfikatorami](artifacts/ci173-search-baseline.json).
To dowód poprzedniego kodu; nowe poprawki #59/#60/#61 wymagają własnego CI i świeżego
niezależnego przeglądu. Nie scalono PR ani nie rozluźniono bramek.

Nadal otwarte: jakość modelu #41/#48, wybór i konfiguracja produkcyjnego dostawcy WWW,
trafność PL/EN, źródła/cytowania, prawdziwe API/hybryda, RTX 2060 12 GB i mikrofon.
[Eksperyment #58](2026-11-10-branch-tool-facts.md) pozostał odrzucony. Okno właściciela:
rozwój przed 04:30 UTC, tylko końcowe testy/raport do 05:00 UTC, potem STOP.
