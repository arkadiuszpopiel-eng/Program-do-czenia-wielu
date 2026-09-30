# Spike (b) — most CLI: zimny start, kanał uprawnień, brak odczytu tokenów

**Cel (PLAN §5.5, §8.5, §16.2 F0 (b); ACCEPTANCE F0-05, F0-06, F0-07):** na desktopie, z Twoim logowaniem do CLI
(bramka #1 — masz je), zmierzyć i potwierdzić:

1. **zimny start** `claude -p "..."` i `codex exec "..."` — czas od uruchomienia do pierwszego bajta odpowiedzi
   i do zakończenia (`Measure-Command`), 10 startów → do ADR (5);
2. **kanał uprawnień:** 100 % próśb o uprawnienia (n ≥ 20) trafia do **naszego** kanału, nie do domyślnego promptu CLI —
   Claude Code przez `--permission-prompt-tool`, Codex przez tryb approvals w app-server;
3. **0 odczytów tokenów:** żaden proces „Alfy" (tu: nasz proces-atrapa mostu) ani sam most **nie czyta** plików
   z sekretami logowania CLI (`~/.claude`, `~/.codex`) poza samym procesem CLI — monitor ETW / Process Monitor.

Kod mostu powstaje w F4; w F0 mierzymy **gotowe CLI** i zachowanie ich flag. Skrypty przygotowuje sesja AI w worktree
`spike/b-most-cli`; tu protokół i szablony. Wymaga: `claude` (Claude Code) i `codex` (Codex CLI) zainstalowane i zalogowane.

## 0. Sprawdzenie logowania

```powershell
claude --version ; codex --version
claude -p "napisz tylko: OK" ; codex exec "print only: OK"
```

Jeśli proszą o logowanie — zaloguj się (bramka #1). Ścieżki sekretów do monitorowania (potwierdź, gdzie są):

```powershell
Get-ChildItem $HOME\.claude, $HOME\.codex -Force -ErrorAction SilentlyContinue | Select-Object FullName
# Codex bywa tez w %USERPROFILE%\.codex\auth.json; Claude Code w ~\.claude\.credentials.json lub Credential Manager
```

## 1. Zimny start (`measure-cold-start.ps1`)

```powershell
pwsh -NoProfile -File .\evals\spikes\b-most-cli\measure-cold-start.ps1 -Runs 10
```

Skrypt dla `claude -p` i `codex exec` (prompt stały, krótki, deterministyczny — „odpowiedz jednym słowem: gotowe"):
`Measure-Command` na całym wywołaniu; jeśli CLI ma tryb strumieniowy (`--output-format stream-json` / `--json`),
mierzy też czas do pierwszego zdarzenia (pierwszy bajt). Pierwszy przebieg = zimny (po restarcie lub `-ClearCache`),
kolejne = ciepłe. Zapisuje `results\<maszyna>-<data>-coldstart.md` i `.csv` z medianą, p95, min/max osobno dla
zimnego i ciepłego. Wynik → ADR (5).

Uwaga: „zimny" tu znaczy pierwszy start po dłuższej przerwie / restarcie (pliki CLI nie w cache dysku). Zapisz w wynikach,
czy był po restarcie.

## 2. Kanał uprawnień (n ≥ 20)

### Claude Code — `--permission-prompt-tool`

Zamiast wbudowanego promptu Claude Code kieruje **każdą** prośbę o uprawnienie do wskazanego narzędzia MCP.
W spike'u sesja AI dostarcza minimalny serwer MCP `permission-logger` z jednym narzędziem, które: (a) loguje każde
wywołanie do `results\perm-claude-<data>.jsonl` (czas, narzędzie, wejście), (b) zwraca decyzję wg parametru
(`-Mode allow` albo `-Mode deny`). Uruchomienie:

```powershell
# serwer MCP w tle (sesja AI dostarcza permission-logger)
pwsh -NoProfile -File .\evals\spikes\b-most-cli\run-permission-probe.ps1 -Cli claude -N 20 -Mode deny
```

Skrypt uruchamia `claude -p` z zadaniem wymagającym ≥ 20 akcji podlegających uprawnieniom (np. „utwórz 20 plików
`probe-01.txt` … `probe-20.txt` w katalogu tymczasowym") i flagą `--permission-prompt-tool mcp__permission-logger__ask`
(dokładna nazwa wg dokumentacji Claude Code — sesja AI potwierdzi). Liczymy: ile próśb trafiło do logu vs. ile akcji
CLI próbowało wykonać. Cel: **20/20 do naszego kanału**, 0 do domyślnego promptu (przy `-Mode deny` żaden plik nie
powstaje — dowód, że decyzja przechodzi przez nas).

### Codex CLI — approvals w app-server

Codex w trybie `app-server` (albo `codex exec` z polityką approvals) zgłasza żądania zatwierdzenia przez protokół
serwera zamiast pytać w terminalu. Sesja AI dostarcza minimalnego klienta app-server, który loguje i odpowiada.
Uruchomienie:

```powershell
pwsh -NoProfile -File .\evals\spikes\b-most-cli\run-permission-probe.ps1 -Cli codex -N 20 -Mode deny
```

Do potwierdzenia w spike'u (PLAN §8.5 „approvals app-server — do potwierdzenia"): czy **wszystkie** klasy akcji
(zapis pliku, shell, sieć) przechodzą przez approvals, czy któraś omija kanał. Zapisz w wynikach mapę: klasa akcji → kanał.

Wynik obu: `results\<maszyna>-<data>-permissions.md` — tabela: CLI, akcji zleconych, próśb w naszym kanale, próśb w
domyślnym promptcie, % przechwycenia.

## 3. Zero odczytów tokenów (`monitor-token-access.ps1`)

Sprawdzamy, że proces mostu/atrapy „Alfy" **nie sięga** po pliki logowania CLI (izolacja „opaque worker", §8.5).
Dwa sposoby — użyj przynajmniej jednego, najlepiej obu.

### Sposób A: Process Monitor (Sysinternals, ręczny, najpewniejszy)

```powershell
winget install --id Microsoft.Sysinternals.ProcessMonitor --source winget
```

1. Uruchom Process Monitor (jako administrator).
2. Filtr (Filter → Filter…): dodaj reguły `Path` **contains** `\.claude` → Include, `Path` contains `\.codex` → Include;
   Operation `is` `CreateFile`/`ReadFile` → Include. Zastosuj.
3. Zacznij przechwytywanie, uruchom scenariusz mostu (atrapa „Alfy" wykonuje zadanie zlecające pracę do CLI).
4. W wynikach: zapisz **każdy** proces, który dotknął tych ścieżek. Oczekiwane: **tylko** `claude.exe` / `codex.exe`
   (i ewentualnie ich `node.exe`), **nigdy** proces atrapy „Alfy".
5. File → Save → CSV do `results\<maszyna>-<data>-procmon.csv` (przefiltrowany).

### Sposób B: ETW przez `monitor-token-access.ps1` (automatyczny)

```powershell
# pwsh jako administrator (sesja ETW File I/O wymaga podniesienia)
pwsh -NoProfile -File .\evals\spikes\b-most-cli\monitor-token-access.ps1 -DurationSec 120 -WatchPaths "$HOME\.claude","$HOME\.codex"
```

Skrypt startuje sesję ETW (dostawca Microsoft-Windows-Kernel-File albo `logman` z `FileIo`), przez `-DurationSec` zbiera
zdarzenia otwarcia/odczytu plików pod `-WatchPaths`, po czym wypisuje tabelę: proces (PID, nazwa, ścieżka exe) × plik ×
operacja. Podczas zbierania **Ty uruchamiasz** scenariusz mostu w drugim oknie. Zapisuje `results\<maszyna>-<data>-etw.csv`.
Kryterium (F0-06): w wynikach **żaden** wiersz z procesem innym niż `claude`/`codex`/ich `node` dla tych ścieżek → **0 odczytów**.

Jeśli ETW File I/O jest zbyt „głośne" lub wymaga sterownika, użyj Sposobu A — jest wystarczający dla F0.

## Szablon wyników — `results/<maszyna>-<data>.md`

```markdown
# Spike (b) — <maszyna> — <RRRR-MM-DD>

CLI: claude <wersja>, codex <wersja> | Windows: <winver> | po restarcie: tak/nie

## Zimny start (n=10)
| CLI | Zimny: do 1. bajta [ms] | Zimny: całość [ms] | Ciepły p50 całość [ms] | p95 | min/max |
|---|---|---|---|---|---|
| claude -p | | | | | |
| codex exec | | | | | |

## Kanał uprawnień
| CLI | Akcji zleconych | Próśb w naszym kanale | Próśb w domyślnym promptcie | % przechwycenia | Klasy akcji poza kanałem |
|---|---|---|---|---|---|
| Claude Code (--permission-prompt-tool) | 20 | | | | |
| Codex (app-server approvals) | 20 | | | | |

## Dostęp do plików logowania (0 odczytów)
| Ścieżka | Procesy, które ją czytały | Czy jakiś ≠ claude/codex? |
|---|---|---|
| ~/.claude | | |
| ~/.codex | | |
Metoda: Procmon / ETW. Wynik F0-06: 0 odczytów przez proces Alfy: tak/nie.

## Wnioski do ADR (5)
Zimny start akceptowalny? Który kanał uprawnień pełny? Co wymaga uwagi w F4?
```

## Co skopiować z powrotem

`results/<maszyna>-<data>.md` + pliki `.csv` (coldstart, permissions, procmon/etw) — wszystkie małe.
Nie kopiuj zawartości `~/.claude` ani `~/.codex` (to sekrety) — tylko fakt, kto je czytał.
