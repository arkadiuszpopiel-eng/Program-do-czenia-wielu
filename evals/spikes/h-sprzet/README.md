# Spike (h) — pomiary na klasach sprzętu

**Cel (PLAN §3.5, §16.2 F0 (h); ACCEPTANCE F0-12, F0-13):**

1. whisper.cpp **Vulkan na RX 9070 XT** (desktop) i **CUDA na RTX 4050** (laptop): czas transkrypcji 60 s próbki PL,
   RTF, szczyt VRAM, wynik `whisper-bench`, **test stabilności 1 h w pętli** (0 crashy);
2. to samo **pod limitami baseline** (6 rdzeni, 16 GB, budżet VRAM 8 GB) z korektą GPU × 2,2 dla desktopu;
3. llama.cpp `llama-bench` — tok/s (pp512 / tg128) dla modeli **4B i 8B Q4_K_M**, Vulkan i CUDA;
4. **RTF Pocket TTS PL** przy 6 rdzeniach i czas do pierwszego fragmentu;
5. (opcjonalnie, jeśli zostanie czas) model KWS „Hej Alfa" — tylko notatka o wykonalności, bez pomiaru.

Wyniki: `results/<maszyna>-<data>.md` według `results/TEMPLATE.md`. Skrypty dopisują wiersze same.

Kryteria go/no-go: **0 crashy w 1 h**, szczyt VRAM ≤ 7 GB (8 GB minus pulpit 0,5–1 GB), RTF whisper turbo Q5 < 0,3
na baseline emulowanym (żeby finalizacja STT mieściła się w 150–300 ms dla krótkich tur), tok/s LLM 4B zapisany
(decyzja o lokalnym LLM w ADR (14)).

## Przygotowanie

### A. Katalogi

```powershell
$S = "$HOME\alfa-spikes"
New-Item -ItemType Directory -Force "$S\bin\whisper-vulkan","$S\bin\whisper-cuda","$S\bin\llama-vulkan","$S\bin\llama-cuda","$S\models","$S\samples" | Out-Null
```

### B. whisper.cpp — gotowe wydania dla Windows

Nie zgaduj numerów wersji. Wejdź na `https://github.com/ggml-org/whisper.cpp/releases` i wybierz **najnowsze
wydanie z serii 1.8.x** (VOICE.md §16: minimum 1.8.1). W sekcji *Assets* tego wydania:

| Maszyna | Szukaj pliku, którego nazwa zawiera | Rozpakuj do |
|---|---|---|
| desktop (AMD) | `vulkan` **i** `x64` (np. `whisper-vulkan-bin-x64.zip` lub podobnie) | `$S\bin\whisper-vulkan` |
| laptop (NVIDIA) | `cublas` lub `cuda` **i** `x64` — plus, jeśli jest osobno, paczkę `cudart-*.zip` z tego samego wydania (biblioteki CUDA runtime; rozpakuj do tego samego folderu) | `$S\bin\whisper-cuda` |
| obie (CPU, porównawczo) | `bin-x64` bez `vulkan`/`cuda` — albo użyj wersji Vulkan/CUDA z flagą `-ng` (bez GPU); skrypt robi to sam dla `-Backend cpu` | — |

Jeśli w wydaniu **nie ma** paczki Vulkan dla Windows, zbuduj ją sama/sam (potrzebny Vulkan SDK, MSVC, CMake z VS Build Tools):

```powershell
cd "$S\bin"
git clone --depth 1 --branch <tag-wydania-1.8.x> https://github.com/ggml-org/whisper.cpp.git whisper-src
cd whisper-src
cmake -B build -DGGML_VULKAN=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j
Copy-Item build\bin\Release\*.exe,build\bin\Release\*.dll "$S\bin\whisper-vulkan"
```

(dla CUDA: `-DGGML_CUDA=ON` i katalog `whisper-cuda`). Sprawdzenie:

```powershell
& "$S\bin\whisper-vulkan\whisper-cli.exe" --help | Select-Object -First 5
```

Potrzebne pliki: `whisper-cli.exe`, `whisper-bench.exe` (w starszych paczkach `main.exe` / `bench.exe`;
skrypt obsługuje obie nazwy) oraz `whisper-stream.exe` (do spike'u (a)).

### C. Model whisper

Oficjalne źródło modeli whisper.cpp to repozytorium `ggerganov/whisper.cpp` na Hugging Face:

```powershell
$url = 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin'
Invoke-WebRequest -Uri $url -OutFile "$S\models\ggml-large-v3-turbo-q5_0.bin"
Get-FileHash "$S\models\ggml-large-v3-turbo-q5_0.bin" -Algorithm SHA256   # zapisz w wynikach
```

Rozmiar ok. 547 MiB. Na desktopie dodatkowo `ggml-large-v3.bin` (pełny, ~3 GiB) — profil D-AMD16, i `ggml-small-q5_1.bin`
(fallback CPU). Ta sama komenda z inną nazwą pliku.

### D. Próbka 60 s PL

Skrypt potrzebuje pliku WAV: **16 kHz, mono, 16-bit PCM**, ~60 s Twojej mowy po polsku (może być nagranie typu 1
z `e-voice-lab/protokol-nagran.md`, przycięte). Konwersja w Audacity: *Ścieżki → Resample 16000*, *Plik → Eksportuj →
WAV 16-bit PCM*, mono. Albo `ffmpeg` (`winget install Gyan.FFmpeg`):

```powershell
ffmpeg -i wejscie.wav -ac 1 -ar 16000 -sample_fmt s16 -t 60 "$S\samples\pl-60s.wav"
```

Do samego testu stabilności wystarczy dowolna mowa PL; do porównań WER — Twoja.

### E. llama.cpp — gotowe wydania

`https://github.com/ggml-org/llama.cpp/releases` — wydania mają numery `bNNNN` (nie wersje semantyczne);
weź **najnowsze**. W *Assets* szukaj:

| Maszyna | Nazwa zawiera | Rozpakuj do |
|---|---|---|
| desktop | `bin-win-vulkan-x64` | `$S\bin\llama-vulkan` |
| laptop | `bin-win-cuda` + `x64` (wersja CUDA 12.x zgodna ze sterownikiem) **oraz** `cudart-llama-bin-win-cuda-*.zip` do tego samego folderu | `$S\bin\llama-cuda` |

Potrzebny plik: `llama-bench.exe` (oraz `llama-cli.exe` / `llama-server.exe` do testu jakości PL w F1).

### F. Modele LLM (GGUF, Q4_K_M — bez kwantów IQ, VOICE.md §16)

Dobór: **4–4,5B** — model z dobrym polskim, np. **Bielik-4.5B-v3.0-Instruct** (SpeakLeash). Oficjalne repozytorium
`speakleash/Bielik-4.5B-v3.0-Instruct-GGUF` ma tylko `Q8_0` (≈ 4,8 GB — model domyślny Alfy) i `fp16`; zmierz `Q8_0`,
a dla porównania z progiem planu także `Q4_K_M` z repozytorium innego autora (zapisz, czyje).
**8B** — np. **Qwen3-8B** (`Qwen/Qwen3-8B-GGUF`, plik `*Q4_K_M*.gguf`) albo **Llama-3.1-8B-Instruct** w Q4_K_M
z repozytorium o dobrej reputacji (bartowski/unsloth). Zasady: pobieraj z oficjalnego lub znanego repozytorium,
zapisz **pełną nazwę pliku i SHA-256** w wynikach; jeśli plik nie ma licencji pozwalającej na redystrybucję,
to nie szkodzi — w spike'u tylko mierzymy.

```powershell
Invoke-WebRequest -Uri '<link do pliku Q4_K_M z zakladki Files and versions>' -OutFile "$S\models\<nazwa>.gguf"
Get-FileHash "$S\models\<nazwa>.gguf" -Algorithm SHA256
```

Orientacyjne rozmiary: 4,5B Q8_0 ≈ 4,8 GB, 4,5B Q4_K_M ≈ 2,8 GB, 8B Q4_K_M ≈ 4,9 GB. Na laptopie (6 GB VRAM) 8B Q4_K_M w całości
na GPU się **nie zmieści** obok STT — to oczekiwany wynik, zapisz, ile warstw (`-ngl`) weszło.

## Pomiar 1 — whisper.cpp (`bench-whisper.ps1`)

Z katalogu głównego repo. Desktop, Vulkan, bez emulacji:

```powershell
pwsh -NoProfile -File .\evals\spikes\h-sprzet\bench-whisper.ps1 -Machine desktop -Backend vulkan -WhisperDir "$HOME\alfa-spikes\bin\whisper-vulkan"
```

Laptop, CUDA:

```powershell
pwsh -NoProfile -File .\evals\spikes\h-sprzet\bench-whisper.ps1 -Machine laptop -Backend cuda -WhisperDir "$HOME\alfa-spikes\bin\whisper-cuda"
```

CPU (porównawczo, górna granica dla baseline):

```powershell
pwsh -NoProfile -File .\evals\spikes\h-sprzet\bench-whisper.ps1 -Machine desktop -Backend cpu -WhisperDir "$HOME\alfa-spikes\bin\whisper-vulkan"
```

Test stabilności 1 h (uruchom **osobno**, na końcu dnia; skrypt transkrybuje próbkę w pętli i liczy błędne wyjścia):

```powershell
pwsh -NoProfile -File .\evals\spikes\h-sprzet\bench-whisper.ps1 -Machine desktop -Backend vulkan -WhisperDir "$HOME\alfa-spikes\bin\whisper-vulkan" -StabilityMinutes 60
```

Parametry: `-Model` (domyślnie `$HOME\alfa-spikes\models\ggml-large-v3-turbo-q5_0.bin`), `-Sample`
(domyślnie `$HOME\alfa-spikes\samples\pl-60s.wav`), `-Threads` (domyślnie 6 = baseline), `-Runs` (5), `-Language` (`pl`).

Co skrypt mierzy: `whisper-bench` (czas enkodera, ms), transkrypcja `Runs` razy — czas mediana/max, **RTF** = czas / długość
próbki, szczyt VRAM (NVIDIA: `nvidia-smi`; AMD: licznik `\GPU Adapter Memory(*)\Dedicated Usage` — ten sam, który
pokazuje Menedżer zadań → Wydajność → GPU → „Pamięć dedykowana GPU"), liczba nieudanych uruchomień w teście stabilności.
Zapisuje pełne wyjście do `results\<maszyna>-<data>-whisper-<backend>.log` i dopisuje wiersz do `results\<maszyna>-<data>.md`.

**Ręczna kontrola VRAM na AMD:** otwórz Menedżer zadań → Wydajność → GPU 0 podczas transkrypcji i zapisz
najwyższą wartość „Pamięć dedykowana GPU" — porównaj z tym, co policzył skrypt.

## Pomiar 2 — llama.cpp (`bench-llama.ps1`)

```powershell
pwsh -NoProfile -File .\evals\spikes\h-sprzet\bench-llama.ps1 -Machine desktop -Backend vulkan -LlamaDir "$HOME\alfa-spikes\bin\llama-vulkan" -Models "$HOME\alfa-spikes\models\<4B>.gguf","$HOME\alfa-spikes\models\<8B>.gguf"
```

Skrypt uruchamia `llama-bench -p 512 -n 128 -r 5` dla każdego modelu, z `-ngl 99` (GPU) lub `-ngl 0` (CPU), zapisuje
wynik w formacie Markdown (`-o md`) do logu i dopisuje wiersz z tok/s (pp512, tg128) i szczytem VRAM.
Dla laptopa spróbuj też `-NGpuLayers 20` dla 8B (częściowy offload przy 6 GB VRAM).

## Pomiar 3 — Pocket TTS PL

Instrukcja krok po kroku: `bench-pocket-tts.md` (Python w venv tylko na czas spike'u, `uv`, 20 zdań, RTF i czas
do pierwszego fragmentu, z affinity 6 rdzeni).

## Emulacja baseline (`emulate-baseline.ps1`)

Baseline = Ryzen 5 5600 (6 rdzeni / 12 wątków), 16 GB RAM, RX 7600 8 GB. Emulacja na Twoich maszynach:

| Ograniczenie | Jak | Uwaga |
|---|---|---|
| **6 rdzeni** | affinity procesu: maska `0xFFF` = pierwsze 12 procesorów logicznych. Na 5700X3D (8c/16t) to 6 rdzeni fizycznych z SMT; na i7-13700H to 6 rdzeni P z HT (E-rdzenie 12–19 wyłączone) | `Start-Process` + `ProcessorAffinity`; potomkowie dziedziczą |
| **16 GB RAM** | Job Object z `JOB_OBJECT_LIMIT_JOB_MEMORY` na drzewo procesów; domyślnie **12 GB** (16 minus ~4 GB na system i pulpit) | limituje **pamięć zatwierdzoną (commit)**, nie fizyczną — przybliżenie; przekroczenie = błąd alokacji w procesie (to chcemy zobaczyć) |
| **VRAM 8 GB** | nie da się sprzętowo ograniczyć; **mierzymy szczyt** i uznajemy za spełnione, gdy ≤ 7 GB (pulpit 0,5–1 GB) | na laptopie 6 GB jest ciaśniej niż baseline — jeśli działa tam, działa na baseline |
| **GPU RX 7600 (288 GB/s) vs RX 9070 XT (~640 GB/s)** | czasy GPU z desktopu (Vulkan) **× 2,2**; czasy CPU z desktopu **× 1,25** (96 MB L3 vs 32 MB) | korekta liczbowa w `results/TEMPLATE.md` (kolumna „po korekcie") |
| **CPU laptopa** | affinity 6 P-rdzeni daje bliższy wynik niż desktop; nie stosuj korekty × 1,25 | |

Użycie — dowolny program pod ograniczeniami:

```powershell
pwsh -NoProfile -File .\evals\spikes\h-sprzet\emulate-baseline.ps1 -- "$HOME\alfa-spikes\bin\whisper-vulkan\whisper-cli.exe" -m "$HOME\alfa-spikes\models\ggml-large-v3-turbo-q5_0.bin" -f "$HOME\alfa-spikes\samples\pl-60s.wav" -l pl -t 6
```

Albo cały skrypt benchmarku pod ograniczeniami (ograniczenia dziedziczą wszystkie procesy potomne):

```powershell
pwsh -NoProfile -File .\evals\spikes\h-sprzet\emulate-baseline.ps1 -- pwsh -NoProfile -File .\evals\spikes\h-sprzet\bench-whisper.ps1 -Machine desktop-emu -Backend vulkan -WhisperDir "$HOME\alfa-spikes\bin\whisper-vulkan"
```

Parametry: `-AffinityMask` (domyślnie `0xFFF`), `-MemoryLimitGB` (12), `-NoMemoryLimit`, `-NoAffinity`.

Alternatywa bez naszego skryptu — **procgov** (Process Governor, `winget install lowleveldesign.ProcessGovernor`):

```powershell
procgov --maxjobmem 12G --affinity 0xFFF -- "<program>" <argumenty>
```

Sprawdzenie, że ograniczenie działa: w trakcie pracy otwórz Menedżer zadań → Szczegóły → prawy przycisk na procesie →
„Ustaw koligację" — powinno być zaznaczone tylko 12 pierwszych procesorów.

## Co skopiować z powrotem

- `results\<maszyna>-<data>.md` (skrypty dopisują wiersze; tabelę Pocket TTS uzupełniasz ręcznie z `bench-pocket-tts.md`),
- `results\*.log` (jeśli < 1 MB; test stabilności może być większy — wtedy tylko końcówkę: `Get-Content plik.log -Tail 200`),
- SHA-256 modeli, tagi wydań whisper.cpp / llama.cpp, wersja sterownika (skrypty zapisują nagłówek).
