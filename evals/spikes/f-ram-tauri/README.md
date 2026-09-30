# Spike (f) — RAM drzewa Tauri/WebView2 (1 i 3 okna) i zimny start

**Cel (PLAN §3.4, §14.7; ACCEPTANCE F0-11):** zmierzyć na desktopie (z emulacją baseline) i laptopie:

1. sumę **Private Working Set całego drzewa procesów** okna „hello" Tauri 2 (proces `alfa-desktop.exe` /
   `Alfa.exe` + wszystkie potomne `msedgewebview2.exe`) w bezczynności — przy **1 oknie** i po otwarciu **3 okien**;
2. **czas zimnego startu** (uruchomienie → widoczne okno główne) i ciepłego startu;
3. dodatkowo: idle CPU drzewa (ma być ≈ 0 %).

Progi wstępne z §3.4 (ostateczne ustala ADR po tym pomiarze):

| Wskaźnik | Cel wstępny |
|---|---|
| Zimny start do okna | ≤ 1,5 s (do zasobnika ≤ 1 s — mierzone w F1, gdy będzie zasobnik) |
| Idle jądra bez okna | ≤ 40 MB Private WS (mierzone w F1; tu tylko drzewo z oknem) |
| Idle z 1 oknem | **do zmierzenia** — WebView2 dominuje; szacunek 300–500 MB |
| Idle z 3 oknami | do zmierzenia; ważne dla pigułki głosowej (F5-12: RAM dodatkowego okna ≤ budżet z F0-11) |
| Bezczynność z otwartym oknem | ~0 % CPU (§14.7) |

## Krok 1 — zbuduj powłokę „hello"

W katalogu głównym repo (pwsh 7):

```powershell
pnpm install
pnpm --filter @alfa/desktop-ui build
cd apps\desktop\src-tauri
cargo tauri build            # ~5–15 min za pierwszym razem
cd ..\..\..
```

Plik wykonywalny powstaje w `apps\desktop\src-tauri\target\release\alfa-desktop.exe`
(instalator NSIS obok w `target\release\bundle\nsis\`). Mierzymy **binarium release**, nie `cargo tauri dev`
(dev ma serwer Vite i devtools — zawyża RAM).

Co, gdy coś nie działa:

| Objaw | Co zrobić |
|---|---|
| `cargo tauri` nie jest rozpoznawane | `cargo install tauri-cli --version "^2" --locked`, nowe okno pwsh; albo `pnpm --filter @alfa/desktop-ui exec tauri build` |
| błąd `link.exe not found` / MSVC | brak VS Build Tools z „Desktop development with C++" — patrz `evals/spikes/README.md` §1 |
| błąd WebView2 przy starcie | `winget install Microsoft.EdgeWebView2Runtime` |
| `pnpm` nie jest rozpoznawane | `corepack enable`, nowe okno |
| build NSIS pobiera coś z sieci | to normalne (Tauri dociąga NSIS przy pierwszym buildzie) |

Jeśli build release się nie uda, a `cargo tauri dev` działa, zmierz dev i **zaznacz to w wynikach** (kolumna `tryb`).

## Krok 2 — pomiar

Skrypt: `measure-ram.ps1`. Uruchom **z katalogu głównego repo**:

```powershell
pwsh -NoProfile -File .\evals\spikes\f-ram-tauri\measure-ram.ps1 -Machine desktop
```

Parametry (wszystkie mają domyślne wartości):

| Parametr | Domyślnie | Znaczenie |
|---|---|---|
| `-Machine` | `desktop` | nazwa maszyny do nazwy pliku wyników (`desktop`, `laptop`, `desktop-emu`, `laptop-emu`) |
| `-ExePath` | `apps\desktop\src-tauri\target\release\alfa-desktop.exe` | ścieżka do binarium |
| `-StartRuns` | `5` | liczba pomiarów startu (pierwszy = zimny, kolejne = ciepłe) |
| `-Samples` | `10` | liczba próbek RAM w każdej fazie (co `-IntervalSec`) |
| `-IntervalSec` | `2` | odstęp między próbkami |
| `-SettleSec` | `20` | ile czekać po starcie, zanim zacznie się próbkowanie (WebView2 „uspokaja się" ~10–20 s) |
| `-NoInteractive` | wyłączone | pomija fazę 3 okien (gdy nie da się otworzyć dodatkowych okien) |
| `-OutDir` | `evals\spikes\f-ram-tauri\results` | dokąd zapisać wyniki |

Przebieg skryptu:

1. **Start:** uruchamia exe `-StartRuns` razy, mierzy czas do pojawienia się okna głównego
   (`Measure-Command` + oczekiwanie na `MainWindowHandle`), zamyka proces. Pierwszy pomiar traktuje jako zimny.
   Żeby był naprawdę zimny (bez plików w pamięci podręcznej), **uruchom skrypt zaraz po restarcie Windows**
   i nie otwieraj wcześniej Alfy ani Edge.
2. **1 okno:** uruchamia exe, czeka `-SettleSec`, pobiera `-Samples` próbek sumy Private Working Set drzewa
   (`Win32_PerfFormattedData_PerfProc_Process.WorkingSetPrivate` dla każdego PID w drzewie) oraz CPU; zapisuje medianę i maksimum.
3. **3 okna:** prosi Cię o ręczne otwarcie dwóch dodatkowych okien (patrz niżej), naciskasz Enter, próbkuje ponownie.
4. Zapisuje `results\<Machine>-<data>.md` i `.csv`, a także wersję Windows, WebView2 i sterownika GPU.

### Jak otworzyć 2 dodatkowe okna w „hello"

Powłoka F0 ma jedno okno. Do spike'u wystarczy jeden z wariantów (wpisz w wynikach, który):

- **Wariant A (preferowany):** sesja AI dodaje w `apps/desktop` tymczasowy skrót `Ctrl+Shift+N` otwierający nowe
  `WebviewWindow` z tą samą stroną (Tauri 2: `WebviewWindowBuilder`). Wtedy po prostu naciskasz skrót 2 razy.
- **Wariant B (bez zmian w kodzie):** uruchom 3 osobne instancje exe (skrypt zrobi to sam, gdy podasz `-ThreeInstances`).
  To **zawyża** wynik (3 procesy główne zamiast 1), ale daje górną granicę; zaznacz `tryb = 3 instancje`.

### Emulacja baseline

Na desktopie zrób drugi przebieg z ograniczeniami (6 rdzeni, limit pamięci) — patrz `h-sprzet/README.md`:

```powershell
pwsh -NoProfile -File .\evals\spikes\h-sprzet\emulate-baseline.ps1 -- pwsh -NoProfile -File .\evals\spikes\f-ram-tauri\measure-ram.ps1 -Machine desktop-emu
```

RAM drzewa nie zależy od affinity, ale czas startu tak; limit pamięci nie powinien tu być osiągnięty.

## Co skopiować z powrotem

- `results\<Machine>-<data>.md` i `.csv` (skrypt tworzy oba),
- jeśli mierzone w trybie dev albo „3 instancje" — jest to w kolumnie `tryb`,
- krótką notatkę, czy w trakcie pomiaru coś było otwarte (przeglądarka itp.).

Szablon wyniku (skrypt generuje taki sam):

| Maszyna | Tryb | Zimny start [ms] | Ciepły start p50 [ms] | 1 okno: Private WS mediana [MB] | 1 okno: max [MB] | 3 okna: mediana [MB] | 3 okna: max [MB] | Idle CPU [%] | Liczba procesów w drzewie | Windows | WebView2 | Sterownik GPU |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
