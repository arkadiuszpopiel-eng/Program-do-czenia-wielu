# Spike (k) — Broker-UI na wyższym poziomie integralności; weryfikacja odporności na wejście syntetyczne

**Cel (PLAN §8.1–8.2, §16.2 F0 (k); ADR 0003; ACCEPTANCE F0-16):** potwierdzić na desktopie dwie własności
bezpieczeństwa okna zatwierdzeń (Broker-UI):

1. usługa w tle (sesja 0, docelowo osobne konto) potrafi **uruchomić natywne okno Broker-UI w sesji użytkownika**
   na **wyższym poziomie integralności (High IL)** niż zwykłe procesy agentek (Medium IL);
2. dzięki **UIPI** (User Interface Privilege Isolation) okno **ignoruje syntetyczne wejście** wysłane z procesu
   o niższym poziomie integralności — oczekiwany, pozytywny wynik testu to **0/100 skutecznych zatwierdzeń** z takiego procesu.
   To jest test obronny: sprawdzamy, że halucynujący albo przejęty proces agentki **nie** kliknie „Zatwierdź" za użytkownika.
3. kontrola: prawdziwe kliknięcie/klawisz użytkownika działa (inaczej test byłby fałszywie „zielony”); a wejście z procesu
   **High IL** przechodzi — dowód, że mierzymy poziom integralności, a nie zepsute okno.

Kod (dwa małe exe w Rust) powstaje w **osobnej sesji AI** w worktree `spike/k-broker-ui` (nie merge'owany do main).
Tu jest protokół i skrypt weryfikujący `verify-uipi-rejection.ps1`, który celowo działa **tylko na oknie tego spike'u**
(po stałym tytule) i służy wyłącznie potwierdzeniu, że izolacja działa.

## Co ma zbudować sesja AI (lista)

| Artefakt | Wymagania |
|---|---|
| `broker-ui-spike.exe` (Rust, `windows-rs`, Win32) | okno 400×200, tytuł stały `Alfa Broker-UI (spike)`, klasa `AlfaBrokerUiSpike`, przycisk „Zatwierdź" (ID 1001) i pole tekstowe (ID 1002); liczy kliknięcia i znaki; **każde** odebrane `WM_COMMAND`/`WM_CHAR` zapisuje do `%LOCALAPPDATA%\alfa-spike-k\broker-ui.log` z czasem i wynikiem `GetCurrentInputMessageSource()` (`originId`: `IMO_HARDWARE` / `IMO_INJECTED` / `IMO_SYSTEM`) — tak wykryjemy wejście wstrzyknięte nawet gdyby przeszło; **bez** manifestu `uiAccess`; w pasku tytułu własny poziom integralności z `GetTokenInformation(TokenIntegrityLevel)`, np. `[High]` |
| `broker-svc-spike.exe` (Rust, usługa) | po starcie: `WTSGetActiveConsoleSessionId` → `WTSQueryUserToken` → duplikat tokena; **wariant 1:** `GetTokenInformation(TokenLinkedToken)` (podniesiony token, gdy użytkownik jest administratorem); **wariant 2:** `SetTokenInformation(TokenIntegrityLevel, High)` na duplikacie tokena użytkownika (High IL bez uprawnień admina — preferowane dla produktu) → `CreateEnvironmentBlock` → `CreateProcessAsUserW` z `lpDesktop = winsta0\default`; błędy do `%ProgramData%\alfa-spike-k\svc.log` |
| `README-build.md` w worktree | build (`cargo build --release`), instalacja/odinstalowanie usługi, sprzątanie |

Oba warianty tokena testujemy; ADR (3) wybiera jeden.

## Przygotowanie (desktop)

```powershell
# 1. Build (zwykly uzytkownik, w worktree spike'u)
cargo build --release
# 2. Kopia poza katalog uzytkownika (usluga ma miec dostep)
New-Item -ItemType Directory -Force 'C:\ProgramData\alfa-spike-k' | Out-Null
Copy-Item .\target\release\broker-ui-spike.exe, .\target\release\broker-svc-spike.exe 'C:\ProgramData\alfa-spike-k\'
# 3. (pwsh jako administrator) rejestracja i start uslugi
sc.exe create AlfaBrokerSpike binPath= 'C:\ProgramData\alfa-spike-k\broker-svc-spike.exe' start= demand
sc.exe start AlfaBrokerSpike
```

Sprawdź, że okno `Alfa Broker-UI (spike) [High]` pojawiło się na pulpicie. Poziom integralności niezależnie —
Process Explorer (Sysinternals, kolumna „Integrity") albo:

```powershell
# Install-Module NtObjectManager -Scope CurrentUser
Get-NtProcess -Name broker-ui-spike.exe | Select-Object ProcessId, IntegrityLevel
```

Ma pokazać `High`, a zwykłe okno (np. Notatnik) `Medium`.

## Test — `verify-uipi-rejection.ps1`

Skrypt uruchamiasz jako **zwykły użytkownik** (Medium IL) — to symuluje proces agentki. Odczytuje licznik okna
Broker-UI (przez jego log), wykonuje N prób „zatwierdzenia" czterema metodami wejścia syntetycznego skierowanego
**wyłącznie na okno o tytule `Alfa Broker-UI (spike)`**, potem czyta licznik ponownie. Oczekiwane: licznik się
**nie zmienił** i log Broker-UI nie zawiera `IMO_INJECTED` przy zdarzeniach zmieniających stan (0/100 przejść).

```powershell
pwsh -NoProfile -File .\evals\spikes\k-broker-ui\verify-uipi-rejection.ps1 -Attempts 100
```

Metody (wszystkie z procesu Medium IL, kierowane na okno spike'u):

| Metoda | API | Czego dowodzi |
|---|---|---|
| A | `SendInput` (klawisze + klik) przez .NET P/Invoke | podstawowy wektor „computer use" agentki |
| B | `SendKeys` (`System.Windows.Forms.SendKeys.SendWait`) po `SetForegroundWindow` | wariant wysokopoziomowy |
| C | `PostMessage(WM_KEYDOWN/WM_CHAR)` na uchwyt pola/przycisku | wektor omijający fokus |
| D | UI Automation `InvokePattern.Invoke()` na przycisku | wektor przez drzewo dostępności |

Kontrola pozytywna (uruchom **raz**, ręcznie): kliknij przycisk fizycznie i wpisz znak — licznik rośnie, log ma
`IMO_HARDWARE`. Druga kontrola: uruchom skrypt z procesu High IL (`-AsHigh`, wymaga sesji AI, która zbuduje mały
launcher High IL) — wtedy A/B przechodzą, co potwierdza, że blokada zależy od poziomu integralności.

## Wynik — `results/<maszyna>-<data>.md`

```markdown
# Spike (k) — <maszyna> — <RRRR-MM-DD>

Windows: <winver, build> | Broker-UI IL: <High?> | wariant tokena usługi: linked / SetTokenIntegrity | gałąź: spike/k-broker-ui@<commit>

| Metoda wejścia z Medium IL | Prób | Skutecznych zatwierdzeń | `IMO_INJECTED` w logu? | Wynik |
|---|---|---|---|---|
| A. SendInput | 100 | | | ma być 0 |
| B. SendKeys | 100 | | | ma być 0 |
| C. PostMessage | 100 | | | ma być 0 |
| D. UIA Invoke | 100 | | | ma być 0 |
| Kontrola: fizyczny klik/klawisz | 5 | | (IMO_HARDWARE) | ma być 5 |
| Kontrola: wejście z High IL | 20 | | | >0 = OK (dowód zależności od IL) |

Kryterium F0-16: kolumny A–D = 0 skutecznych. Wariant tokena wybrany do ADR (3): ...
Uwagi (czy High IL bez admina wystarcza, koszt, problemy z CreateProcessAsUser):
```

## Sprzątanie

```powershell
# (administrator)
sc.exe stop AlfaBrokerSpike ; sc.exe delete AlfaBrokerSpike
Remove-Item -Recurse -Force 'C:\ProgramData\alfa-spike-k', "$env:LOCALAPPDATA\alfa-spike-k"
```

## Co skopiować z powrotem

Plik `results/<maszyna>-<data>.md` z wypełnioną tabelą oraz końcówkę `broker-ui.log`
(`Get-Content "$env:LOCALAPPDATA\alfa-spike-k\broker-ui.log" -Tail 50`) jako dowód wartości `originId`.
