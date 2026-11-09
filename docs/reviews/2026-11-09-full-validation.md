# Pełne próby Rust i UI na169a6004

9.11.2026, źródło `169a6004b414d9fa610f10fdacde565de3bd2410`.

| Zakres | Zaliczone | Nieudane | Pominięte |
| --- | --- | --- | --- |
| Cloud Linux Rust, domyślne cechy, cały workspace z app-core partiami |3492 |0 |24 |
| Windows Rust CI159, `--all-features` |3516 |5 |45 |
| Chromium CI159, E2E z atrapą backendu |890 |0 |0 |
| UI jednostkowe CI159,103 pliki |573 |0 |0 |
| Windows/WebView2, rehearsal80 |60 |1 |6 |

## Cloud Linux Rust

Runner `/workspace/scratch/run-rust-final.py`, zakończony03:15UTC, kod0.
Przez cały przebieg HEAD i źródła Rust były zamrożone. Model CPU i testy czasowe UI pozostawały
zatrzymane. Rust1.94.1; target `/workspace/target-nightly`, profile test/dev bez debug-info,
`CARGO_INCREMENTAL=0`, dwa zadania kompilacji. Najpierw:

```sh
cargo test --locked --workspace --exclude app-core --no-fail-fast -j2
cargo test --locked -p app-core --lib --no-fail-fast -j2
```

Następnie wszystkie77 integracji odczytane z `cargo metadata --locked --no-deps`, po4 cele
na komendę `cargo test --locked -p app-core --no-fail-fast -j2 --test …`; między partiami
`cargo clean -p app-core`, żeby utrzymać zapas dysku. Na końcu `cargo test --locked -p app-core --doc -j2`.
Wszystkie komendy zakończyły się kodem0. Poza app-core3285/0/22; app-core207/0/2.
To zakres domyślnych cech wybranych pakietów; Windows `--all-features` jest osobną próbą.

[Komendy, czasy i SHA256 wszystkich logów](artifacts/rust-final-2026-11-09.json).
Surowe logi: `/workspace/scratch/rust-final/`. Ostrzeżenie nieużywanych `KillReason` / `KillSwitch`
w chronionym teście Brokera pozostało. Testy przeszły, ale Clippy workspace nadal blokuje ten problem.
Gotowa chroniona łatka wymaga przeglądu człowieka; nie zastosowano jej do publikowanego kodu.

## Windows CI159

[Przebieg](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42549736330),
zadanie128942473689. Komenda `cargo test --workspace --all-features --no-fail-fast` ukończyła cały
zestaw; cztery cele miały łącznie pięć nieudanych testów. Zliczono1121 oryginalnych wierszy wyników;
7 powtórzeń z końcowego podsumowania grep wykluczono. [Metadane](artifacts/ci159-windows-results.json).

- `conductor_handoff::later_refusal_does_not_erase_recorded_command_output` i
  `receipts_survive_limit_and_core_restart_without_repeating_delegate`: niedokończona tura/20s.
- `files::attachment_taint_reaches_broker_and_survives_restart`: `kernel_broker()` zwrócił `None`,
  mimo wiersza modułu `ready`. Zwolnienie uchwytów #39 nie rozwiązało problemu; przyczyna otwarta.
- `exec_low::powershell_runs_at_low_integrity`: utrata polskich znaków w fixture pliku.
- `shell_utf8::cmd_output_is_decoded_from_the_oem_code_page`: strona OEM runnera daje
  „zazólc gesla jazn” zamiast „zażółć gęślą jaźń”.

Oba testy `queued_message_*` przeszły, w tym nowy przypadek wygasania, który zawieszał CI156.
Poprawka #39 ma zatem potwierdzenie Windows bez zmiany300ms. Test własnej Luny po restarcie
i ponownym przypisaniu roli również przeszedł; to próba z atrapą dostawcy, osobno od Bielika.

## UI i granice dowodów

CI159 zadanie128942473626:890/890 E2E w28,2min,573/573 jednostkowych,103 pliki, typy0/0,
lint/build/bundle/Storybook zielone. Obejmuje zegar funkcjonalnego Planu #38 i3 próby viewportu #40.
Historyczny lokalny877/2 pozostaje zachowany. Tauri, cargo-deny i dokumentacja także zielone;
Linux Clippy zablokowany chronionymi importami.

[Windows80 i diagnostyka #42](2026-11-09-webview-child-lock.md): dokowanie przeszło39,9s,
całość60/1/6 z timeoutem kłódki Dziecko. Emulacja1600×900 dotyczyła tylko fragmentu dokowania.
Log powłoki wskazuje deweloperskiego Sędziego w procesie aplikacji; nie jest to odbiór usługi
na osobnym koncie. Mikrofon, sprzęt właściciela i Windowsowy odsłuch pozostają niesprawdzone.
Wersja0.0.85, etapJ otwarty; niezależny przegląd i zielone CI są nadal wymagane przed scaleniem.
