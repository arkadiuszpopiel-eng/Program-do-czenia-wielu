# Dane testowe zależne od hosta — Linux Cloud

Karty [#27](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/27),
[#28](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/28),
[#29](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/29),
[#30](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/30).
Baza opublikowana `efd1ab62`; Rust 1.94.1, Linux x86_64. Przyrost dotyczy danych i asercji testów.

## Przyczyny i zakres

- `app-agents`: ścieżka `C:\Users\A` nie jest bezwzględna na Linux. Test rozwijania ustawienia
  NextGO używa ścieżki tymczasowej hosta, zachowuje odrzucanie względnych i nieznanych zmiennych.
  Przykłady dysków Windows mają własny test Windows; inny test potwierdza ich odrzucenie poza Windows.
- `tools-fs-impl`: test separatorów Windows pozostaje na Windows. Nowy test na składnikach hosta
  sprawdza zgodność ścieżki, różne nazwy/katalogi i rozróżnianie liter poza Windows.
- `tools-system-impl`: atrapa wykrywania programów buduje ścieżkę przez `PathBuf::join`, dzięki czemu
  `file_stem` rozpoznaje nazwę na każdym hoście. Wersje Node, Git, Python Launcher i winget są sprawdzane.
- `app-core`: ta sama korekta ścieżki atrapy CLI. Stan konta, plan, redakcja e-maila/organizacji
  i lista wywołanych metod atrapy zachowują wszystkie asercje. Nie uruchamia prawdziwych mostów.
  Test katalogu wymaga modeli wspólnych i sprawdza obecność binarki Vulkan dokładnie dla Windows x64;
  na pozostałych hostach wymaga jej braku. Testy instalacji Windows z #26 nadal działają osobno.

Produkcji nie rozszerzono o akceptowanie ścieżek Windows na Linux, ignorowanie wielkości liter
na Linux ani dobieranie katalogu Windows zamiast hosta. Nie zmieniano polityk, evals i progów.

## Sprawdzenie

Pierwotna pełna próba na `9b8b7416` miała tu sześć nieudanych przypadków: po jednym w trzech
modułach oraz dwa testy CLI i jeden katalogu w app-core. Wszystkie są w zachowanym raporcie pełnego Rust.

`cargo test --locked -p app-agents --no-fail-fast -j 2` na `576f16d6`:
**32 zaliczone / 0 nieudanych / 3 pominięte**; Clippy `--all-targets -- -D warnings` zaliczone.
Sprawdzony katalog Git `crates/app-agents`: `a61b6ea5690e85f997362704ab9a70ca8e2449ed`.
Pominięte: prawdziwy git/cargo, ręczny pomiar lokalnego modelu, wersje programów prawdziwej maszyny.
Kompilacja osobnego grafu funkcji wymagała nowych artefaktów; nie działał równocześnie model CPU.

Na `4de47089`:

| Polecenie / zakres | Wynik |
| --- | --- |
| `cargo test --locked -p tools-fs-impl -p tools-system-impl --no-fail-fast -j 2` | **56 zaliczonych / 0 nieudanych / 0 pominiętych** |
| Clippy obu modułów `--all-targets -- -D warnings` | zaliczone |
| `cargo test --locked -p app-core --test bridges_check --test commands --no-fail-fast -j 2` | **7 zaliczonych / 0 nieudanych / 0 pominiętych** |
| Clippy app-core `--lib --test bridges_check --test commands -- -D warnings` | zaliczone |

Pierwszy przebieg #28–29 miał **55 zaliczonych / 1 nieudany**: poprawa wersji ujawniła dalszą
asercję Edge. Atrapa na Linux tworzyła inny plik niż pełna względna ścieżka Windows w katalogu
przeglądarek. Utworzono dokładny plik tego katalogu w folderze tymczasowym; produkcyjna lista
ścieżek i jej interpretacja pozostają dotychczasowe. Pierwszy log zachowano jako
`fs-system-portability-first.log`. Nie pominięto testu ani asercji przeglądarki.

Łącznie ukierunkowane zakresy wyżej: **95 zaliczonych / 0 nieudanych / 3 pominięte**.
Pełny Rust po przyroście nie był jeszcze ponowiony; nie odejmowano matematycznie porażek od
pierwotnego zestawu3461/20/24. Dalszy znany przypadek `tool_diag` ma kartę
[#33](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/33).

[Wyniki maszynowe](artifacts/linux-fixtures-results.json). Logi: `/workspace/scratch/nextgo-path-{tests,clippy}.log`,
`fs-system-portability-{tests,clippy}.log`, `core-portability-{tests,clippy}.log`.

Kontrola rozmiaru wykazała początkowo412 linii w `tools-fs-impl/src/core.rs` przy limicie400.
Moduł testów wpisów wydzielono do `core_entry_tests.rs`, zachowując nazwy testów i wszystkie
asercje. Powtórka obu modułów na `fc7e8fa7`: **56/56**, Clippy zaliczone, kontrola rozmiaru:
3481 plików,264 crate'y,0 naruszeń, bez nowych wyjątków. Format Rust i dokumentacja sprawdzone.
Logi końcowe: `fs-system-portability-final-{tests,clippy}.log`.

Niezależny przegląd i Windows CI pozostają otwarte. Te testy nie są odbiorem sprzętowym.
