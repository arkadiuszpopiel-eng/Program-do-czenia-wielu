# ui-terminal — SPEC (v1: kontrakt + implementacja, F4)

## Cel
Wbudowany terminal ConPTY, w którym **użytkownik sam** loguje się do CLI mostów (`claude` → `/login`, `codex login`) i używa powłoki (PLAN §5.5–5.6, bramka ludzka #1). Sterowany wyłącznie przez użytkownika; agentki nie mają do niego dostępu; strumień nie jest logowany ani zapisywany (mogą się w nim pojawić tokeny).

## Fala i priorytet
F4, P1 (krok „mosty CLI” w onboardingu).

## Kontrakt
```rust
pub trait TerminalService {                   // tylko komendy UI w korzeniu kompozycji (app-*)
    fn open(&self, OpenRequest { profile: Shell|Cmd|ClaudeLogin|CodexLogin, size: PtySize, cwd? }, &UserGesture, Arc<dyn TerminalSink>) -> Result<TerminalId, TerminalError>;
    fn input(&self, TerminalId, &[u8], &UserGesture) -> Result<(), TerminalError>;   // ≤ 64 KiB
    fn resize(&self, TerminalId, PtySize) -> Result<(), TerminalError>;
    fn close(&self, TerminalId) -> Result<(), TerminalError>;                        // zabija drzewo procesów
    fn list(&self) -> Vec<TerminalInfo { id, profile, pid, alive }>;
}
pub trait TerminalSink { fn output(&self, TerminalId, &[u8]); fn exited(&self, TerminalId, Option<i32>); }
pub mod ui_only { pub fn user_gesture() -> UserGesture }   // tylko w obsłudze komendy UI
// platform-contract: PseudoConsolePort::spawn(PtySpec) → PtySession { take_output, write_input, resize, exit_code, close }
// impl: platform-windows-pty-impl::WinPty (CreatePseudoConsole + Job Object), atrapa: platform-fake::FakePty
```
Zdarzenia magistrali (bez treści): `terminal.opened` (`terminal`, `profile`, `pid`), `terminal.exited` (`terminal`, `code`), `terminal.closed` (`terminal`).

### Do dodania w UI/`app-*` (inna sesja)
Komendy: `terminal_open { profile, cols, rows, cwd?, channel }` → `{ terminal }`; `terminal_input { terminal, data_b64 }` (xterm.js `onData`); `terminal_resize { terminal, cols, rows }`; `terminal_close { terminal }`; `terminal_list {}`. Strumień: `tauri::ipc::Channel<TerminalFrame>` przekazany w `terminal_open` (`{kind:"output", data_b64}` | `{kind:"exit", code}`) — `TerminalSink` zapisuje **tylko** do kanału; nigdy przez `app-api` events/bus/log. `UserGesture` tworzy wyłącznie handler komendy. Programy profili z wykrycia mostów (`accounts-hub`); `default_cwd` = `%USERPROFILE%`.

## Zależności
`platform-contract` (`PseudoConsolePort`, `filter_env`), `core-bus-contract` (zdarzenia cyklu życia). Celowo **bez** `tools-*`, `agent-*`, `mcp-*`, `memory-*`, `core-log-*`, `tracing`.

## Niezmienniki
- Brak API dla agentek: żaden `Tool`/`Toolset`; od `ui-terminal-*` zależy tylko `app-*` (test `tests/isolation.rs`); okno terminala to WebView Alfy — `tools-input` go nie dotknie (strażnik celów).
- Strumień tylko do `TerminalSink`; bufor odczytu zerowany po zakończeniu; `Debug` bez treści; zdarzenia bez treści (test szpiegowski `tests/terminal.rs`: token z ekranu logowania i wpisane hasło nie występują w zdarzeniach ani `Debug`).
- Środowisko: `filter_env(…, DEFAULT_ENV_ALLOWLIST)` — bez kluczy API/tokenów Alfy — + `TERM=xterm-256color`, `COLORTERM=truecolor`.
- Koniec procesu wykrywany (wątek nadzoru) → pseudokonsola zamknięta → UI dostaje `exited`; zamknięcie i porzucenie menedżera zabijają drzewo (Job Object `KILL_ON_JOB_CLOSE` + `TerminateJobObject`).
- Alfa nie analizuje strumienia (stan „zalogowano” sprawdza `agent-backends` przez oficjalne polecenie statusu CLI).
- Terminal nigdy nie startuje z wyzwalacza ani harmonogramu (tylko gest użytkownika).

## Zdolności / uprawnienia
Brak (nie przechodzi przez Brokera — to wejście właściciela, nie agentki).

## Izolacja
`inproc`, `on-demand`; na sesję: wątek wyjścia + wątek nadzoru (200 ms).

## Budżet zasobów
RAM ≤ 4 MB + conhost na sesję; ≤ 3 sesje naraz.

## Konfiguracja (klucze TOML)
`[ui.terminal] max_sessions = 3`, `default_cwd`, `[ui.terminal.programs] shell/cmd/claude/codex` (z wykrycia).

## Wkład do UI
Panel terminala (xterm.js) w kreatorze kont i kroku onboardingu „mosty CLI” (F4); ikona „zakończono” po `exit`.

## Testy akceptacyjne
- `ACC-F4-ui-terminal-01`: brak API dla agentek (graf zależności + brak `Tool`).
- `ACC-F4-ui-terminal-02`: test szpiegowski — 0 wystąpień tokenu/hasła w zdarzeniach, `Debug`, środowisku.
- `ACC-F4-ui-terminal-03`: zamknięcie zabija drzewo (`platform-windows-pty-impl/tests/conpty_windows.rs`: `ping` zabity ≤ 5 s).
- Bramka ludzka #1: logowanie do CLI w terminalu na desktopie.

## Fake
`ui-terminal-fake::FakeTerminals` (echo, `emit`, zamknięcie z `exited`); `platform-fake::FakePty` (wyjście skryptowane, `exit`, `tree_killed`).

## Otwarte pytania
- Kopiowanie zaznaczenia ze schowka w terminalu — tylko UI (bez historii schowka dla tej treści).
- WSL/SSH jako profile — P2.

## Utwardzenia po przeglądzie #2 (2026-10)
- **P2-05:** ConPTY (`platform-windows-pty-impl`) — lista atrybutów w buforze wyrównanym, zapis wejścia bez trzymania zamka uchwytu (zamknięcie terminala nie czeka na zawieszony `WriteFile`).
