# Powtarzalne fixture'y rdzenia na Windows, #45

Pełne CI159 iCI160 zachowały te same trzy porażki app-core: dwa `conductor_handoff`
oraz `files::attachment_taint_reaches_broker_and_survives_restart`. CI160974ae45e miało
3516 zaliczonych /5 nieudanych /46 pominiętych (dwie pozostałe porażki to kodowanie platformy).

## Przekazywanie wyników

Dwa testy demonstracyjnego wyjścia PowerShell miały badać zachowanie dowodów po odmowie modelu
i wznowieniu tury. Faktycznie uruchamiały powłokę hosta i nie kończyły tury w20s.
Teraz używają `FakeExec` z kontrolowanym stdout tej samej komendy `Write-Output`.
Komenda, odmowa, limit kroków, raport Krytyczki i wszystkie asercje treści pozostają.
Dodatkowe asercje wymagają dokładnie jednego wykonania, również po restarcie.
„541” w fixture jest znacznikiem demonstracyjnego wyniku, nie wynikiem kompilacji Alfy.

Testy działają także na Linux, bez rzeczywistego PowerShell. Rzeczywiste procesy, UTF-8
i niska integralność pozostają sprawdzane oddzielnie w `platform-windows-impl`.
Nie zmieniono20s, budżetów, roli Krytyczki ani polityki Brokera.

## Skażenie po restarcie

Samo zwolnienie uchwytów #39 nie rozwiązało błędu Windows: `kernel_broker()` nadal zwracał `None`,
mimo wiersza modułu `ready`. Fixture ma teraz trzy fazy w osobnych pełnych runtime Tokio;
koniec runtime zatrzymuje zadania w tle przed ponownym otwarciem tych samych danych.
Między fazami przechodzą tylko identyfikator rozmowy i ten sam sejf testowy.
Pozostają rzeczywisty Broker, tekstowy załącznik, asercje `TaintedEgress` przed i po wysyłce,
odtworzenie przy otwarciu oraz odtworzenie przy turze bez otwierania. Błąd inicjalizacji
podaje fazę, by odróżnić pierwszy start od obu restartów. Przyczynę Windows potwierdzi następne CI.

## Sprawdzenie

Rust1.94.1, Cloud Linux, jawne atrapy wykonania/audio:

```sh
cargo test --locked --no-fail-fast -p app-core --test conductor_handoff --test files -j2
cargo clippy --locked -p app-core --lib --test conductor_handoff --test files -j2 -- -D warnings
```

**4/4 conductor i3/3 files**,0 nieudanych; odpowiednio6,27s i1,38s po kompilacji3m11s.
Ukierunkowany Clippy bez ostrzeżeń,1m49s; format i rozmiary sprawdzone osobno.
Logi `/workspace/scratch/windows-core-fixtures-{tests,clippy}.log`.
Źródła produkcyjne, chronione moduły i progi evals pozostają dotychczasowe.
To nie zaliczenie Windows ani odbiór sprzętu; rzeczywisty Windows po publikacji jest wymagany.
