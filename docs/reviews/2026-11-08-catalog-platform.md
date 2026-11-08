# Katalog modeli i pełne testy Linux — 8.11.2026

Karta [#26](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/26).
Baza: `25c4e788`. Sprawdzony katalog Git modułu `crates/app-models`:
`794d0e4b5c4947b685e047d7e2c071c33e339f06`. Linux Cloud, Rust 1.94.1.

## Przyczyna i poprawka

13 testów `app-models` w pełnej próbie Linux oczekiwało składników Windows, używając `builtin()`
filtrowanego dla hosta. Dodano `builtin_for(Option<Target>)`; dotychczasowe `builtin()` nadal wybiera
host. Testy kart, licencji, pakietów, CUDA i archiwów jawnie wybierają Windows x64.

Metadane archiwów Windows wymagają `.exe` także przy odczycie na Linux. Nazwy ręcznych instalacji
(np. FFmpeg) zależą od wybranego celu. Testy potwierdzają Windows, Linux, macOS i nieznany cel,
brak binarek Windows w katalogu Linux oraz właściwe mapowanie nazw przez `AppPaths::sidecar`.
Nie dodano binarek Linux do katalogu ani nie zmieniono licencji, potwierdzeń, hashy czy fixture'ów UI.

## Sprawdzenie poprawki

| Polecenie | Wynik |
| --- | --- |
| `cargo test --locked -p app-models --no-fail-fast -j 2` | **78 zaliczonych, 0 nieudanych, 1 pominięty** |
| `cargo clippy --locked -p app-models --all-targets -j 2 -- -D warnings` | zaliczone |
| `cargo fmt -p app-models` / `git diff --check` / rozmiary plików | zaliczone |

Testy odtwarzają instalację ze sztucznych archiwów i lokalnego serwera. Nie uruchamiają binarek Windows
w Linuxie ani nie dowodzą działania na sprzęcie właściciela. Pierwsza próba tej poprawki ujawniła jeszcze
asercję szukającą nazwy Linux po instalacji sztucznego archiwum Windows; poprawiono ją, zachowując
kontrolę wszystkich plików i osobno mapowania hosta. Trzy nowe testy pilnują granic platform i nazw.

Logi: `/workspace/scratch/catalog-platform-tests-final.log`, `catalog-platform-clippy-final.log`.
Przegląd w świeżej sesji i pełne CI najnowszego przyrostu pozostają otwarte. Wersja 0.0.85.

## Pełna próba sprzed poprawki

Źródła `9b8b7416`; [wyniki maszynowe](artifacts/full-linux-2026-11-08.json).

- UI Playwright/Chromium: **856/856, 48,6 min**, axe i cztery style. Jest to pierwsza kompletna próba;
  wcześniejszy log kończył się po 434 zaliczonych przypadkach, bez podsumowania.
- Rust: **3461 zaliczonych, 20 nieudanych, 24 pominięte**. Workspace bez app-core oraz wszystkie
  76 celów integracyjnych app-core, biblioteka i doctesty. Partie po osiem i `cargo clean -p app-core`
  między partiami pozwoliły zmieścić się na dysku 32 GB. Samo wyjście skryptu sterującego nie jest
  zaliczeniem zestawu — raport sumuje wyniki każdego polecenia.
- 13 porażek dotyczyło katalogu poprawionego wyżej. Pozostałe: ścieżki hosta w teście NextGO [#27](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/27),
  porównanie separatorów [#28](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/28),
  atrapa wersji oprogramowania [#29](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/29),
  dwa przypadki atrapy CLI [#30](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/30),
  oczekiwanie składników Windows w `app-core/tests/commands.rs` oraz atrapy `media_convert` / `vision_ocr`
  w jednym teście `tool_diag`. Nie uznano ich za zielone ani nie zmieniono progów.

Nie wykonano jeszcze ponownego całego Rust po tej poprawce. Próba Bielika i powtórzenie lokalnego
budżetu wyszukiwania mają osobną kolejność po zakończeniu kompilacji. Windows/WebView2 z GitHub CI
i prawdziwy mikrofon pozostają odrębnymi dowodami.
