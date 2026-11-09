# Fixture'y kodowania Windows, #46

CI159 iCI160 miały dwie porażki: `powershell_runs_at_low_integrity` oraz
`cmd_output_is_decoded_from_the_oem_code_page`. To poprawki danych i operacji testowych;
kod produkcyjny dekodera, progi czasu i polityka niskiej integralności pozostają dotychczasowe.

## PowerShell i plik

Windows PowerShell5.1 zapisywał plik domyślnym ANSI, przez co „zażółć” stawało się „zazólc”.
`Set-Content` i `Get-Content` używają teraz jawnego UTF8. Zachowano polskie znaki w asercji,
wykonanie o niskiej integralności, odmowę zapisu obok katalogu roboczego i brak pliku poza nim.

## CMD i OEM

Pełny polski tekst nie jest reprezentowalny na przykład w OEM437 runnera. Fixture odczytuje
publiczną konfigurację OEM i używa kodera .NET z `EncoderExceptionFallback`: preferuje polski
tekst, następnie znak spoza ASCII możliwy do zakodowania bez zamian best-fit. Numer strony,
oryginalny tekst UTF8 i zakodowane bajty przechodzą jako ASCII/hex przez istniejący port PowerShell.
Oczekiwana wartość pochodzi z oryginalnego tekstu emitera, niezależnie od badanego dekodera Core.

CMD nadal wykonuje zwykłe `echo`. Test porównuje surowe bajty z kodowaniem emitera, wymaga wejścia
niepoprawnego jako UTF8 przy zwykłej stronie OEM i sprawdza odzyskanie oryginalnego tekstu.
Natywne OEM65001 jest obsługiwane jako UTF8. Istniejący test PowerShell z całym polskim tekstem
„zażółć gęślą jaźń” pozostaje bez zmiany. Nie zastąpiono oczekiwanej wartości wynikiem dekodera
ani samym ASCII; nie pominięto testu.

## Sprawdzenie

Rust1.94.1, kontrola z Linux dla `x86_64-pc-windows-msvc`:

```sh
cargo +1.94.1 check --locked -p platform-windows-impl \
  --test exec_low --test shell_utf8 --target x86_64-pc-windows-msvc -j2
cargo +1.94.1 clippy --locked -p platform-windows-impl \
  --test exec_low --test shell_utf8 --target x86_64-pc-windows-msvc -j2 -- -D warnings
```

Obie komendy zaliczone,0 ostrzeżeń; ciepły check0,87s, Clippy12,60s. Format i rozmiary
sprawdzone osobno. To kompilacja/kontrola typów i lint, **nie wykonanie Windows**;
rzeczywisty PowerShell/CMD i asercje niskiej integralności wymagają następnego Windows CI.

Pierwszy prototyp bezpośredniego Win32 został odrzucony przez `unsafe-code` (2 błędy).
Zastąpiono go bezpiecznym portem procesu; nie dodano wyjątku ani nie zmieniono konfiguracji lintów.
Dwie próby uruchomienia Clippy były zablokowane przez brak komponentu lub targetu we właściwym
aliasie toolchain; końcowe polecenie korzysta jawnie z1.94.1 z dostępnymi składnikami.
Nie dodano crate'ów ani nie zmieniono Cargo.lock.

Logi `/workspace/scratch/windows-encoding-{check,check-safe,clippy,clippy-stable,clippy-final}.log`.
