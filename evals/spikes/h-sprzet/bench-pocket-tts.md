# Pocket TTS PL — pomiar RTF i czasu do pierwszego fragmentu (spike (h), także (e))

**Cel:** zmierzyć na desktopie (affinity 6 rdzeni = emulacja baseline) i laptopie, ile Pocket TTS z polskim modelem
społeczności potrzebuje na syntezę 20 zdań PL: **RTF** (czas syntezy / długość audio; cel deklarowany ≈ 0,21) i
**czas do pierwszego fragmentu** (cel ~200 ms; budżet TTFB TTS w §6.4: 200–500 ms lokalnie).

Python jest tu **tylko na czas spike'u** — produkt nie używa Pythona (ADR 0004). Wszystko idzie do jednego
katalogu `$HOME\alfa-spikes\pocket-tts`, który po spike'u można usunąć.

## 1. Instalacja (venv przez `uv`)

```powershell
winget install --id astral-sh.uv --source winget
# nowe okno pwsh:
$P = "$HOME\alfa-spikes\pocket-tts"
New-Item -ItemType Directory -Force $P | Out-Null
cd $P
uv venv --python 3.12 .venv
.\.venv\Scripts\Activate.ps1
uv pip install pocket-tts numpy soundfile
```

Jeśli `uv` sprawia kłopot, wariant z `pip`:

```powershell
winget install --id Python.Python.3.12 --source winget
py -3.12 -m venv .venv ; .\.venv\Scripts\Activate.ps1
python -m pip install --upgrade pip ; pip install pocket-tts numpy soundfile
```

Sprawdź, jak nazywa się pakiet i jaki ma interfejs — **repozytorium Pocket TTS (Kyutai) na GitHub → README**.
Nazwa pakietu na PyPI i nazwy funkcji mogły się zmienić; skrypt niżej ma oznaczone miejsca do dopasowania.

## 2. Model PL społeczności

W PLAN §6.2 mowa o „Pocket TTS + model PL społeczności (CC-BY-4.0)". Znajdź go na Hugging Face: szukaj
`pocket-tts` + `polish`/`pl` w nazwie, sprawdź kartę modelu (licencja, jak ładować). Zapisz w wynikach:
identyfikator repozytorium HF, commit/rewizję i licencję. Jeśli model PL wymaga innego sposobu ładowania niż
domyślny (np. ścieżka do wag), dopasuj sekcję `# --- DOPASUJ ---` w skrypcie.

Głos referencyjny (do klonu z krótkiej referencji): 5–10 s **Twojego** nagrania w ciszy (WAV 24 kHz lub 48 kHz, mono)
albo próbka głosu z modelu, jeśli ma wbudowane. Nazwa w wynikach: `ref-wlasny` / `ref-wbudowany`.

## 3. Skrypt pomiarowy

Plik `bench_pocket_tts.py` leży obok tego README. Skopiuj go do `$P` i uruchom:

```powershell
Copy-Item .\evals\spikes\h-sprzet\bench_pocket_tts.py $P\
cd $P
python bench_pocket_tts.py --ref "$HOME\alfa-spikes\samples\ref-10s.wav" --out "$HOME\Alfa\evals\spikes\h-sprzet\results\<maszyna>-<data>-pocket.csv"
```

Zdania: skrypt ma wbudowane te same 20 zdań PL, co `e-voice-lab/candidates.md` (żeby ocena odsłuchu i RTF
dotyczyły tych samych tekstów). Wyjścia WAV (do odsłuchu w spike'u (e)) trafiają do `$P\out\*.wav`.

Co mierzy dla każdego zdania: czas do pierwszego fragmentu audio (jeśli pakiet ma API strumieniowe), czas całkowity,
długość audio, RTF = czas / długość; na końcu medianę, p95, max; szczyt RSS procesu (RAM).

## 4. Pod ograniczeniami baseline (6 rdzeni)

Desktop — obowiązkowo z affinity (laptop: opcjonalnie, bo ma 6 rdzeni P):

```powershell
cd $HOME\Alfa
pwsh -NoProfile -File .\evals\spikes\h-sprzet\emulate-baseline.ps1 -- "$P\.venv\Scripts\python.exe" "$P\bench_pocket_tts.py" --ref "$HOME\alfa-spikes\samples\ref-10s.wav" --out ".\evals\spikes\h-sprzet\results\desktop-emu-<data>-pocket.csv"
```

Ogranicz też wątki bibliotek numerycznych, żeby nie walczyły o 12 wątków logicznych: przed uruchomieniem
`$env:OMP_NUM_THREADS = '6'; $env:MKL_NUM_THREADS = '6'` (i porównawczo `12`).

## 5. Co skopiować z powrotem

Skrypt wypisuje na końcu gotowy wiersz tabeli Markdown — wklej go do sekcji „Pocket TTS PL" w
`results\<maszyna>-<data>.md` (szablon `results/TEMPLATE.md`), a plik `.csv` zostaw w `results\`.
Dodaj: wersję pakietu (`pip show pocket-tts`), identyfikator modelu PL i rewizję, rodzaj referencji głosu,
`OMP_NUM_THREADS`, czy z affinity.

Do spike'u (e): 20 plików WAV z `$P\out` skopiuj do `evals\corpus\tts-candidates\pocket-pl\` (poza gitem) —
posłużą do ślepej oceny.
