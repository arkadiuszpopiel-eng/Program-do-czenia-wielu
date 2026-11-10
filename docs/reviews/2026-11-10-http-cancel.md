# Stop na granicach HTTP i zatwierdzenia pobrania — 10.11.2026

Karta [#62](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/62),
baza `5fe2edd7bb3c48d11f1cd4da8a2a9a14f7f6746a`, worktree `/workspace/wt-http-cancel`.
Kontynuacja [planu 55](../plan/55-enterprise-lokalnie-chmura-i-wyszukiwanie.md).

## Odtworzone problemy

Adapter HTTP ustawiający Stop w ostatnim poll przed oddaniem gotowych nagłówków nadal
mógł zwrócić udany HEAD. Podobnie EOF mógł zakończyć GET sukcesem z treścią, a fragment
przekraczający limit dać udane obcięcie mimo ustawionego tokenu anulowania.
Pobieranie po EOF lub po końcowym flush mogło zatwierdzić plik mimo Stop.

Testowy port ustawia token bezpośrednio przed Ready, bez losowania, sleep lub obciążenia CPU.
Magazyn w pamięci rejestruje otwarte, porzucone i zatwierdzone ujścia. Siedem testów przed
poprawką nie przeszło; dwie kontrole zwykłego żądania i pobrania już zatwierdzonego przeszły.

## Naprawa i granica efektu

Core::send i next_chunk sprawdzają token przed wywołaniem adaptera i po odpowiedzi, również
po błędzie lub EOF. GET/HEAD sprawdza go po zwolnieniu zgód i przed oddaniem wyniku.
Anulowanie nie staje się sukcesem, obcięciem lub błędem I/O; odrzucona treść nie trafia do payloadu.

Po ostatnim zapisie pobranie sprawdza token wewnątrz wątku blokującego, przed wywołaniem
commit, także gdy zadanie czekało na uruchomienie w puli. Porzucone ujście sprząta część
dotychczasowym Drop. Test wymaga braku zatwierdzonego pliku i otwartych ujść oraz poprawnego
licznika porzuceń. Stop po udanym zatwierdzeniu zachowuje prawdziwy wynik oraz ślad pliku.
Nie usuwamy arbitralnie pliku już opublikowanego. Nie przerywamy synchronicznego commit w środku;
kontrola wyznacza granicę przed jego rozpoczęciem. Nie jest to pomiar dysku właściciela.

Komunikat przyznaje: „Żądanie sieciowe mogło już dotrzeć do odbiorcy”. Anulowanie odbioru nie
dowodzi braku egress lub zatrzymania serwera. Zwolnienie zgód pozostaje w istniejącej ścieżce.
Nie zmieniono polityk, progów, timeoutów, zależności, schematów, Brokera, watchdog lub kill-switch.

## Wyniki lokalne

| Kontrola | Wynik |
| --- | --- |
| Pierwsza kompilacja fixture | kod 101, błędny typ DenyLists; testy niewykonane, log zachowany |
| Właściwy RED po poprawieniu fixture | **2 zaliczone / 7 nieudanych / 0 ignorowanych**, kod 101 |
| GREEN http_cancel | **9/0/0**, 13 wariantów |
| Pełne tools-net-contract/impl/fake, all-features | **56/0/0**, kod 0, 12 wyników celów z doctestami |
| Clippy all-targets/all-features | kod 0, bez ostrzeżeń, 19,64 s |
| Format i lint | kody 0, bez naruszeń |

Nowe przypadki: HEAD/nagłówki, GET/EOF, download/EOF, Stop podczas ostatniego flush,
zatrzymany fragment przekraczający limit, brak rozpoczęcia ujścia po zatrzymanych nagłówkach,
błąd nagłówków ze Stop dla fetch/download oraz zwykłe GET/HEAD/download/błąd bez Stop.
Kontrola po commit wymaga zachowania prawdziwego wyniku i danych pliku. Dotychczasowe
testy wyszukiwania, sekretów, limitów, przekierowań i pobierania także przeszły.

```sh
export CARGO_TARGET_DIR=/workspace/target-nightly
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo +1.94.1 test --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-features -j2
cargo +1.94.1 clippy --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-targets --all-features -j2 -- -D warnings
cargo +1.94 fmt --all -- --check
pnpm lint
```

[Komendy, kody, wyniki i hashe](artifacts/http-cancel-validation.json).
Logi: `/workspace/scratch/http-cancel-boundaries/`. To atrapy HTTP, DownloadStore i harness
Brokera na Linux Cloud. Nie wykonywano ruchu internetowego, nie odczytywano sekretów,
nie badano rzeczywistego dysku Windows, GPU lub mikrofonu.

## Baza i dalszy odbiór

CI 174 i Windows 96 dla 5fe2edd7 są zakończone sukcesem: 10/10 i 4/4 zadań.
Linux **3535/0/26**, Windows **3561/0/47**, po 1130 wyników celów. Chromium **915/915**
w częściach 305/305/305 (8,3/8,5/9,4 min), Windows/WebView2 **63/0/5** w 2,9 min.
[Zapis z identyfikatorami](artifacts/ci174-http-baseline.json).
Pełny wynik zachowano przed publikacją. Nowy przyrost wymaga własnego CI i świeżego
niezależnego przeglądu; nie scalono PR i nie podnoszono limitów.

Nadal otwarte: wybór dostawcy WWW, rzeczywiste API/hybryda, trafność i cytowania, GPU RTX2060
12GB i mikrofon. #41/#48 nierozwiązane; Bielik nie zaliczył prawdziwości potwierdzeń akcji,
a kandydat #58 pozostał wycofany.
