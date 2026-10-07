# evals/spikes — harness pomiarowy spike'ów F0 (do uruchomienia na Windows)

Ten katalog to **instrukcje i skrypty pomiarowe** dla spike'ów fali 0 z `docs/PLAN.md` §16.2
(wiersz F0) i progów z `docs/ACCEPTANCE.md` §3 (F0-03 … F0-16). Uruchamiasz je Ty, na swoich
maszynach z Windows 11, a wyniki wklejasz z powrotem do repo. Kod pomiarowy sam nie buduje
programu — mierzy gotowe binaria (whisper.cpp, llama.cpp, Pocket TTS, Tauri „hello").

Wyniki zapisujemy w `evals/spikes/<x>/results/<maszyna>-<data>.md` (patrz „Zbieranie wyników").
Na ich podstawie sesja AI uzupełnia ADR-y (3, 4, 5, 11) i budżety §3.4 / §14.7.

## Maszyny i nazwy

| Nazwa w plikach | Sprzęt | Ścieżka GPU |
|---|---|---|
| `desktop` | Ryzen 7 5700X3D · RX 9070 XT 16 GB · 32 GB RAM · Win11 | **Vulkan** (AMD, bez CUDA) |
| `laptop` | i7-13700H · RTX 4050 6 GB · 16 GB RAM · Win11 | **CUDA** (i Vulkan porównawczo) |
| `desktop-emu` | desktop z ograniczeniami baseline (6 rdzeni / 16 GB / VRAM 8 GB) | Vulkan × korekta 2,2 |
| `laptop-emu` | laptop z affinity 6 rdzeni P | CUDA |

Baseline (PLAN §3.4–3.5): Ryzen 5 5600 · RX 7600 8 GB · 16 GB. Nie masz go fizycznie, więc
emulujemy limity (`h-sprzet/emulate-baseline.ps1`) i stosujemy korekty: **czasy GPU Vulkan z desktopu × 2,2**,
**czasy CPU z desktopu × 1,25** (V-Cache zawyża wynik na korzyść).

## Kolejność (PLAN §4.5a)

| Krok | Spike | Katalog | Czas | Wynik trafia do |
|---|---|---|---|---|
| 1 | (f) RAM i zimny start drzewa Tauri/WebView2, 1 i 3 okna | `f-ram-tauri/` | ~1 h | budżety §3.4, §14.7; F0-11 |
| 2 | (h) pomiary sprzętowe: whisper.cpp, llama.cpp, Pocket TTS, stabilność 1 h | `h-sprzet/` | ~1 dzień (w tym 2 × 1 h testu stabilności) | ADR (4), ADR (11); F0-12, F0-13 |
| 3 | (e) Voice Lab PL: tabela kandydatów + ślepa ocena + nagranie korpusu | `e-voice-lab/` | 2–3 h + ~45 min nagrań | ADR (4), ADR (11); F0-08…F0-10; bramka #3 |
| 4 | (a) pętla głosowa mic → VAD → whisper → Pocket-PL → głośnik, barge-in | `a-petla-glosowa/` | ~pół dnia (po (h) i (e)) | ADR (11); F0-03, F0-04 |
| 5 | (j) powłoka Windows: Snap Layouts, Mica, pisownia PL, toasty AUMID | `j-powloka/` | ~2 h | ADR (1)/(7) uwagi; F0-15 |
| 6 | (k) Broker-UI na wyższym poziomie integralności + odrzucenie SendInput | `k-broker-ui/` | ~2 h (po zbudowaniu exe w osobnej sesji) | ADR (3); F0-16 |
| 7 | (b) most CLI: zimny start, `--permission-prompt-tool`, approvals, 0 odczytów tokenów | `b-most-cli/` | ~2 h | ADR (5); F0-05…F0-07 |

Spike (i) (SQLCipher + sqlite-vec + FTS5) nie wymaga Windows — zrobiony w chmurze: `crates/spike-data` (test w CI) i wynik w `evals/spikes/i-dane/RESULT.md`.

## Wymagania wstępne (bramka ludzka #8 — przygotowanie maszyn)

Wszystko poniżej wykonujesz w **PowerShell 7** (`pwsh`) uruchomionym **jako zwykły użytkownik**
(chyba że napisano inaczej). Polecenia wklejasz po jednym i czekasz na zakończenie.

### 0. PowerShell 7 i polityka skryptów

```powershell
winget install --id Microsoft.PowerShell --source winget
# Zamknij i otwórz nowe okno "PowerShell 7" (nie "Windows PowerShell"), potem:
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

Jeśli skrypt z repo odmawia uruchomienia („nie jest podpisany cyfrowo"), odblokuj pliki:

```powershell
Get-ChildItem -Recurse -Filter *.ps1 .\evals\spikes | Unblock-File
```

### 1. Toolchainy (obie maszyny)

```powershell
winget install --id Git.Git --source winget
winget install --id Rustlang.Rustup --source winget
winget install --id Microsoft.VisualStudio.2022.BuildTools --source winget --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id OpenJS.NodeJS.LTS --source winget
winget install --id Microsoft.EdgeWebView2Runtime --source winget
winget install --id KhronosGroup.VulkanSDK --source winget
```

Po instalacji **zamknij i otwórz nowe okno pwsh**, a potem sprawdź:

```powershell
git --version
rustup default stable-x86_64-pc-windows-msvc
rustup toolchain install 1.94 ; rustup show          # repo przypina 1.94 w rust-toolchain.toml
cargo --version
node -v                                              # wymagane >= 22
corepack enable ; corepack prepare pnpm@10.33.0 --activate ; pnpm -v
cargo install tauri-cli --version "^2" --locked
cargo install cargo-deny --locked
$env:VULKAN_SDK                                      # ma pokazać ścieżkę, np. C:\VulkanSDK\1.x.y.z
```

Jeśli `node -v` pokazuje wersję inną niż 22.x, to też jest w porządku, o ile jest `>= 22`.

### 2. Tylko laptop (NVIDIA): CUDA Toolkit

```powershell
winget install --id Nvidia.CUDA --source winget
# nowe okno pwsh:
nvcc --version
nvidia-smi
```

Gotowe binaria whisper.cpp/llama.cpp „cuda" wymagają zgodnej wersji bibliotek `cudart` — patrz `h-sprzet/README.md`.

### 3. Przypięcie sterowników GPU (żeby pomiary były powtarzalne)

Zapisz wersję sterownika w każdym pliku wyników (skrypty próbują ją odczytać same).

- **Desktop (AMD Adrenalin):** zainstaluj aktualną wersję ze strony AMD (nie z Windows Update),
  w Adrenalin → Ustawienia → System → wyłącz „Automatyczne aktualizacje" (ustaw „Powiadamiaj").
  Sprawdź wersję: `Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion`.
- **Laptop (NVIDIA):** zainstaluj sterownik Game Ready lub Studio ze strony NVIDIA, w aplikacji NVIDIA
  wyłącz automatyczne pobieranie. Wersja: `nvidia-smi --query-gpu=driver_version --format=csv,noheader`.
- **Obie maszyny — zablokuj sterowniki z Windows Update** (uruchom pwsh **jako administrator**):

```powershell
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate' -Force | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate' -Name 'ExcludeWUDriversInQualityUpdate' -Type DWord -Value 1
```

(cofnięcie: ta sama komenda z `-Value 0`).

### 4. Repo

```powershell
cd $HOME
git clone <adres-repo-prywatnego> Alfa
cd Alfa
pnpm install
```

Wszystkie skrypty zakładają, że uruchamiasz je **z katalogu głównego repo** (tam gdzie `Cargo.toml`).

### 5. Katalog na modele i próbki (poza repo, poza gitem)

```powershell
New-Item -ItemType Directory -Force "$HOME\alfa-spikes\models","$HOME\alfa-spikes\bin","$HOME\alfa-spikes\samples" | Out-Null
```

Do `models` trafiają pliki `.bin` / `.gguf`, do `bin` rozpakowane wydania whisper.cpp / llama.cpp,
do `samples` próbki WAV. `.gitignore` repo i tak wyklucza `*.gguf` i `*.wav`, ale trzymanie ich
poza repo jest prostsze.

## Emulacja baseline — skrót

Szczegóły i skrypt: `h-sprzet/README.md` (sekcja „Emulacja baseline") i `h-sprzet/emulate-baseline.ps1`.
W skrócie: proces testowy uruchamiasz przez `emulate-baseline.ps1 -- <program> <argumenty>`; skrypt
ustawia affinity na 6 rdzeni fizycznych z SMT (maska `0xFFF`) i limit pamięci przez Job Object;
VRAM 8 GB egzekwujemy pomiarem (szczyt ≤ 7 GB, bo pulpit zajmuje 0,5–1 GB), a wyniki GPU z desktopu
mnożymy × 2,2.

## Zbieranie wyników („co skopiować z powrotem")

1. Każdy skrypt zapisuje pliki do `evals/spikes/<x>/results/`:
   - `<maszyna>-<data>.md` — tabela gotowa do wklejenia/commitu (data w formacie `RRRR-MM-DD`),
   - `<maszyna>-<data>.csv` — surowe liczby (też do commitu, są małe),
   - ewentualnie `*.log` z pełnym wyjściem programów (do commitu, jeśli < 1 MB).
2. Dla spike'ów bez skryptu (a, e, j, k, część b) kopiujesz `results/TEMPLATE.md` (lub szablon z README)
   do `results/<maszyna>-<data>.md` i wypełniasz ręcznie.
3. Sprawdź, że w wynikach nie ma niczego prywatnego (ścieżki z nazwiskiem są OK; nagrania i klucze — nie).
4. Wklej w oknie sesji AI treść plików `.md` **albo** zrób commit na gałęzi `spikes/f0-wyniki`:

```powershell
git checkout -b spikes/f0-wyniki
git add evals/spikes/*/results/*.md evals/spikes/*/results/*.csv
git commit -m "spikes F0: wyniki <maszyna> <data>"
git push -u origin spikes/f0-wyniki
```

5. Nagrania głosu (`e-voice-lab/protokol-nagran.md`) zostają w `evals/corpus/` **poza gitem**.

## Wspólne zasady pomiaru (VOICE.md §15.1)

- N ≥ 5 powtórzeń dla wszystkiego, co niedeterministyczne; skrypty domyślnie robią 5.
- Zapisuj wersje: binariów (tag wydania), modeli (nazwa pliku + SHA-256), sterownika GPU, Windows (`winver`).
- Zamknij przeglądarki, Discorda, gry, aktualizacje w tle. Laptop **na zasilaczu**, plan „Najwyższa wydajność";
  osobny przebieg „na baterii" tylko tam, gdzie README to wskazuje.
- Nie uruchamiaj dwóch benchmarków naraz.
