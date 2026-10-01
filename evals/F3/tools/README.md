# Eval narzędzi F3 — fs/shell (`tasks.json`)

Zestaw zadań dla agentki z narzędziami (ACCEPTANCE F3-07; w ACCEPTANCE katalog nazywa się
`evals/F3/tools-local/` — ten zestaw jest jego źródłem, nazwę ujednolica człowiek przy zamrożeniu
hashem). Każde zadanie ma polecenie po polsku, stan początkowy katalogu roboczego i sprawdzenie
stanu końcowego (pliki, treść, brak plików, katalogi, pliki niezmienione, fragmenty odpowiedzi).

Runner: `crates/app-agents/src/eval/` — zadanie przechodzi przez `agent-runtime` (persona Delta
w roli Wykonawczyni, katalog roboczy = katalog zadania) z tymi samymi narzędziami `tools-fs`
i `tools-shell` co aplikacja, przez Brokera (silnik `safety-broker-impl`, polityka bazowa,
zgoda bez okna Brokera wygasa po 1 s = odmowa) i dziennik cofania.

## Format (`version: 1`)

| Pole | Znaczenie |
|---|---|
| `id` | `fs-NN-…` / `sh-NN-…`, unikalne |
| `kind` | `fs` (tylko narzędzia plikowe) albo `shell` (PowerShell — wymaga Windows) |
| `goal` | polecenie dla agentki |
| `setup.files` / `setup.dirs` | stan początkowy (ścieżki względne, bez `..`) |
| `expect.files` | plik musi istnieć; opcjonalnie `equals` (po obcięciu białych znaków), `contains`, `not_contains` |
| `expect.absent` / `expect.dirs` / `expect.unchanged` | ścieżki, których nie ma; katalogi; pliki ze stanu początkowego bez zmian |
| `expect.answer_contains` | fragmenty odpowiedzi końcowej (bez rozróżniania wielkości liter) |
| `ci_script` / `ci_answer` | skrypt atrapy modelu dla CI (argumenty tylko ASCII) |

Porównania treści normalizują UTF-16 LE z BOM (Windows PowerShell 5 `Out-File`), BOM UTF-8 i
końce linii CRLF.

## Uruchomienie

CI (każdy system, atrapa modelu — format + zadania z `ci_script`):

```bash
CARGO_INCREMENTAL=0 cargo test -p app-agents --test eval_tools
```

Pomiar na modelu lokalnym (Windows, `llama-server` z llama.cpp i model pobrany przez Alfę):

```powershell
$env:ALFA_EVAL_LLAMA_SERVER = "$env:LOCALAPPDATA\Alfa\sidecars\llama\llama-server.exe"
$env:ALFA_EVAL_MODELS_DIR   = "$env:LOCALAPPDATA\Alfa\models"
$env:ALFA_EVAL_MODEL        = "bielik-4.5b-v3.0-instruct-q4_k_m"   # opcjonalnie
$env:ALFA_EVAL_REPORT       = "wynik-F3-tools.md"                   # opcjonalnie
cargo test -p app-agents --test eval_tools --release -- --ignored --nocapture
```

`ALFA_EVAL_ONLY=fs` albo `shell` zawęża zestaw. Raport Markdown: zaliczone ogółem i per rodzaj,
tabela zadań (wynik, kroki, wywołania narzędzi, czas, niespełnione warunki). Próg zaliczenia
F3-07 ustala człowiek (ADR 14); zmiana zadań po zamrożeniu wymaga jego zatwierdzenia.
