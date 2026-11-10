# tools-dev — SPEC (v0, C6.5)

## Cel
Narzędzia programisty agentek w katalogu roboczym sesji: odczyt repozytorium Git i uruchamianie
testów projektu z czytelnym podsumowaniem — żeby agentka mogła pisać programy i sprawdzać je
testami bez ręcznego składania poleceń powłoki.

## Kontrakt (`crates/tools-dev-contract`)
```
dev_git { action: status|diff|log|branches|show, path?, staged?, limit? (1–200), rev? } → tekst + GitStatus (dla status)
dev_test { runner?: cargo|npm|pnpm|yarn|pytest|go|dotnet, filter?, timeout_s? (1–1800) } → TestSummary
  { runner, command, passed, failed, skipped, failures[], exit_code, timed_out, ok, elapsed_ms }
```
- Wykrycie projektu (`detect_runner`): Cargo.toml → cargo; package.json ze skryptem `test` →
  pnpm (pnpm-lock.yaml) / yarn (yarn.lock) / npm; go.mod → go; *.sln/*.csproj → dotnet;
  pytest.ini / pyproject.toml / setup.cfg / tox.ini / `tests` → pytest.
- Parsery podsumowań: cargo, pytest, vitest/jest, go, dotnet (z nazwami niezaliczonych).
- `TestSummary.ok` wymaga kodu0, rozpoznanego podsumowania, co najmniej jednego zaliczonego
  testu i braku niezaliczonych. Pusty wynik, nierozpoznany format, filtr bez testów lub same
  pominięcia dają `ok=false` i jawny komunikat „zaliczenie niepotwierdzone”. Samo wykonanie
  narzędzia nadal ma `ToolStatus::Ok`; kod procesu znajduje się też w tekście dla modelu.
- Argumenty z modelu: `safe_token` (litery, cyfry, `._/~:@-`, bez `-` na początku, bez `..`,
  bez `^`), `safe_path` (ścieżka względna, bez `..`, dysku i metaznaków; w cudzysłowie).

## Bezpieczeństwo
`dev_git` — tylko odczyt, zgoda `fs.read(katalog roboczy)`; `dev_test` — `shell.exec(katalog
roboczy)` (testy wykonują kod projektu). Polecenia przez `cmd.exe` w Job Object (limit czasu,
anulowanie, rejestr kill-switcha), środowisko z listy dozwolonej (+ zmienne toolchainów), bez
kluczy; deny-lista katalogów; wynik niezaufany (taint `file`), sekrety redagowane. Zatwierdzanie
i wypychanie zmian — przez `shell_run` z jego zgodami (nie w tym module).

## Testy
Kontraktowe (`contract-tests`), atrapa `tools-dev-fake`, impl na `FakeExec` + Broker
(`tests/dev.rs`), na żywo: `app-agents/tests/dev_live.rs` (prawdziwy git i cargo).
