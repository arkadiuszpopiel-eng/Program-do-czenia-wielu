# tools-shell — SPEC (szkic v0)

## Cel
Wykonywanie poleceń przez agentki: PowerShell 7/5, cmd, ConPTY (strumień wyjścia, interaktywność ograniczona), z tokenem `shell.exec(zakres)`, snapshotem zakresu przed wykonaniem (shadow-git/kopia), limitami czasu/wyjścia, Job Object i możliwością „uruchom w terminalu" z bloku kodu (PLAN §7.2, §8.7, §14.8). WSL2/SSH/Git/Docker — F6.

## Fala i priorytet
F3 (PowerShell/cmd przez ConPTY, snapshot zakresu). P0. `ui-terminal` (logowanie do CLI) — F4, osobny moduł.

## Kontrakt (szkic Rust)
```rust
// tools-shell-contract — SZKIC
pub struct ShellCall { pub shell: Shell /* Pwsh7 | Pwsh5 | Cmd */, pub command: String, pub cwd: PathBuf, pub env: Vec<(String, String)>, pub timeout: Duration,
                       pub scope: Scope /* katalogi, których polecenie może dotykać */, pub interactive: bool }
pub enum ShellEvent { Started { pid }, Stdout(Bytes), Stderr(Bytes), Exited { code, elapsed }, TimedOut, Killed(KillReason) }
pub struct ShellCallCtx { pub token: CapToken, pub session: SessionId, pub run: RunId, pub step: StepId, pub snapshot: Option<SnapshotId> }
pub trait ToolsShell: Send + Sync {
    fn spec(&self) -> ToolSpec;                                      // reversible: Scoped
    fn facts(&self, call: &ShellCall) -> ActionFacts;                // heurystyki destrukcyjności (rm/format/reg delete…), egress, zakres
    fn run(&self, call: ShellCall, ctx: ShellCallCtx, cancel: CancelToken) -> BoxStream<ShellEvent>;
    fn open_in_terminal(&self, call: ShellCall, ctx: ShellCallCtx) -> Result<TerminalHandle>;   // „uruchom w terminalu” (widoczne okno ConPTY w UI)
}
```
Zdarzenia (Narzędzia i GUI + Audyt): `tool.shell.started`, `tool.shell.output` (limit, redakcja), `tool.shell.exited`, `tool.shell.killed`, `tool.shell.out_of_scope_ask`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (`ProcessPort`: ConPTY, Job Object, restricted token), `safety-broker-contract`, `undo-journal-contract` (snapshot zakresu), `risk-classifier-contract`, `tools-fs-contract` (deny-listy).

## Niezmienniki
- Brak wykonania bez tokenu `shell.exec` obejmującego `cwd` i `scope`; polecenie dotykające zakresu poza tokenem = `Ask` (Broker) przed startem.
- Snapshot zakresu przed wykonaniem (PLAN §8.7); brak snapshotu = brak wykonania dla `Scoped`.
- Proces w Job Object (limit CPU/RAM/czasu, zabijanie drzewa), restricted token / AppContainer dla ≤ L3; brak dziedziczenia sekretów w env (redakcja/allowlista zmiennych).
- Deny-lista ścieżek poświadczeń i domen dostawców egzekwowana także dla poleceń (heurystyki + egress-allowlista Jądra).
- Wyjście jest niezaufanym wejściem (taint); ograniczone rozmiarem, reszta do artefaktu; redakcja sekretów w logach.
- Kill-switch zabija wszystkie powłoki (Job Objects) < 200 ms.
- Brak `sudo`-podobnej usługi: elewacja wyłącznie przez Broker/UAC (`system.admin`).

## Zdolności / uprawnienia
`shell.exec(zakres)`; ewentualnie `net.egress(host)` dla poleceń sieciowych (wykrywane heurystyką → osobny token lub `Ask`).

## Izolacja
`inproc` (kontroler) + procesy potomne (`process`, Job Object), `lazy`.

## Budżet zasobów
Start powłoki ≤ 300 ms (pwsh7 cold start bywa wolniejszy — mierzone); RAM kontrolera ≤ 3 MB; limit wyjścia domyślnie 1 MB.

## Konfiguracja (klucze TOML)
`[tools.shell] default = "pwsh7"`, `timeout = "120s"`, `output_max_kb = 1024`, `env_allowlist = ["PATH", "TEMP", ...]`, `job.max_ram_mb = 2048`, `snapshot_required = true` (kernel_policy).

## Wkład do UI
Krok narzędzia z podglądem wyjścia (zwinięty, przewijany), „uruchom w terminalu" w bloku kodu, karta „czeka na zatwierdzenie" dla poleceń poza zakresem, Ustawienia → Komputer.

## Testy akceptacyjne
- `ACC-F3-tools-shell-01`: snapshot zakresu → skrypt modyfikujący pliki → cofnięcie przywraca zakres (z `undo-journal`).
- `ACC-F3-tools-shell-02`: ≥ 100 poleceń poza zakresem / na deny-liście / czytających `~/.claude` = 0 wykonanych bez zatwierdzenia; ETW: 0 odczytów ścieżek poświadczeń.
- `ACC-F3-tools-shell-03`: timeout i kill-switch zabijają drzewo procesów (w tym potomków) 100/100; eval narzędzi shell na lokalnym modelu ≥ próg z F0.

## Fake
`tools-shell-fake`: skryptowane wyniki poleceń (stdout/stderr/kod, opóźnienia z wirtualnym zegarem) bez uruchamiania procesów.

## Otwarte pytania
- Heurystyki destrukcyjności poleceń (lista wzorców) — `THREAT_MODEL.md`; do ustalenia w SPEC v1.
- Interaktywne polecenia (pytania o hasło) — polityka: przerwanie i `Ask` czy zakaz; do ustalenia w SPEC v1.
