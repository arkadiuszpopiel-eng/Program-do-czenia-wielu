# tools-shell — SPEC (v0, zaimplementowany)

## Cel
Wykonywanie poleceń przez agentki: PowerShell 7/5, cmd, ConPTY (strumień wyjścia, interaktywność ograniczona), z tokenem `shell.exec(zakres)`, snapshotem zakresu przed wykonaniem (shadow-git/kopia), limitami czasu/wyjścia, Job Object i możliwością „uruchom w terminalu" z bloku kodu (PLAN §7.2, §8.7, §14.8). WSL2/SSH/Git/Docker — F6.

## Fala i priorytet
F3 (PowerShell/cmd przez ConPTY, snapshot zakresu). P0. `ui-terminal` (logowanie do CLI) — F4, osobny moduł.

## Kontrakt
Format wspólny: `tools-common` (docs/modules/tools-common/SPEC.md).
```rust
// tools-shell-contract
shell_run { command, cwd? /* domyślnie katalog roboczy */, shell?: pwsh | powershell | cmd, timeout_s? }   // reversible: scoped
shell_terminal { command, cwd?, shell? }   // intencja shell.open_in_terminal — właściciel uruchamia sam, nic nie jest wykonywane
pub struct ShellToolsConfig { default_shell, pwsh_path, powershell_path, cmd_path, timeout_default_s, timeout_max_s,
                              output_max_bytes, output_max_chars, memory_limit_mb, env_allowlist }
// tools-shell-impl
pub struct ShellTools; impl ShellTools { pub fn new(deps: ShellToolsDeps /* broker, journal, exec: ExecPort, jobs: JobRegistry, env, deny, config, base_env, bus */) -> Self }
```
Zdolności: `shell.exec(zakres)` (polecenie trafia do reguł Jądra/klasyfikatora), `net.egress(host)` dla poleceń sieciowych (polecenie sieciowe bez jawnego hosta = odmowa `Policy`). Przed wykonaniem snapshot zakresu w dzienniku cofania (`UndoRef { service: Journal }`); proces w Job Object zarejestrowanym w `JobRegistry` (kill-switch Brokera). Wyjście niezaufane (`TaintSource::Tool`), redagowane i obcinane.

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
Krok w wątku i Replay (polecenie, skrócone wyjście, kod wyjścia), karta „czeka na zatwierdzenie” dla poleceń poza zakresem/sieciowych, karta intencji „Uruchom w terminalu” (polecenie do skopiowania, `agents_open_terminal` otwiera Windows Terminal/pwsh w `cwd` bez wykonania), Ustawienia → Komputer.

## Integracja w aplikacji
`app-agents::AgentTools` składa `ShellTools` z tą samą instancją `ExecPort`, którą Broker zabija procesy (`JobRegistry` = silnik Brokera): kill-switch (`Ctrl+Shift+F12`, „stop wszystko” głosem) i Stop/Esc przebiegu kończą drzewa procesów. Bez okna Brokera prośba o zgodę wygasa po ≤ 60 s (odmowa z powodem dla modelu).

## Testy akceptacyjne
- `ACC-F3-tools-shell-01`: snapshot zakresu → skrypt modyfikujący pliki → cofnięcie przywraca zakres (z `undo-journal`).
- `ACC-F3-tools-shell-02`: ≥ 100 poleceń poza zakresem / na deny-liście / czytających `~/.claude` = 0 wykonanych bez zatwierdzenia; ETW: 0 odczytów ścieżek poświadczeń.
- `ACC-F3-tools-shell-03`: timeout i kill-switch zabijają drzewo procesów (w tym potomków) 100/100 (`app-core/tests/agents.rs` na atrapie; sprzętowo — runner self-hosted); eval narzędzi shell na lokalnym modelu ≥ próg z F0 (`evals/F3/tools/`, zadania `sh-*`).

## Fake
`tools-shell-fake`: skryptowane wyniki poleceń; testy aplikacji — `platform-fake::FakeExec` (wyniki, procesy „wiszące” do testów anulowania i kill-switcha).

## Otwarte pytania
- Heurystyki destrukcyjności poleceń (lista wzorców) — `THREAT_MODEL.md`; do ustalenia w SPEC v1.
- Interaktywne polecenia (pytania o hasło) — polityka: przerwanie i `Ask` czy zakaz; do ustalenia w SPEC v1.

## Poprawki po recenzji PR #1 (2026-10-04)
- **Q-1:** katalog roboczy polecenia i ścieżki z polecenia sprawdzane na deny-liście także po rozwiązaniu dowiązań
  (`paths::protected_with_links`). Test: `tools-shell-impl/tests/links.rs`.
- **Q-8:** analiza (`CommandAnalysis`) zbiera też ścieżki względne z `..` i ścieżki od korzenia dysku (`\x`); nowe pole
  `opaque_targets` — polecenie usuwające z celem nieustalonym statycznie (zmienna `$x`, splat `@x`, czasownik usuwania
  za potokiem). Wykonawca ustala cel każdej ścieżki względem `cwd` (`paths::resolve_path_dots`, wieloznacznik
  w ostatnim segmencie → jego katalog); cel poza `cwd` → osobna zdolność `shell.exec` (nieodwracalna). Cel
  nieustalony (zmienna, nieznana zmienna środowiskowa, `..` ponad korzeń, wieloznacznik wyżej) przy poleceniu
  usuwającym → `Destructiveness::Permanent`, `reversible = no`, `bulk = u32::MAX` (najgorszy przypadek: zgoda
  właściciela do L3) — nigdy „cofalne”. Testy: `analysis::tests::delete_targets_parent_variables_and_pipeline`,
  `tools-shell-impl/tests/run.rs::parent_or_unresolved_delete_targets_need_approval`.
