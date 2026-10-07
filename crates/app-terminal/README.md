# app-terminal

Wbudowany terminal w aplikacji (kategoria `app-*`).

- `TerminalApp` — komendy `terminal_open` / `terminal_input` / `terminal_resize` / `terminal_close` /
  `terminal_list` nad `ui-terminal-impl` (ConPTY z `platform-windows-pty-impl`, w testach `FakePty`).
  `UserGesture` powstaje **wyłącznie** w obsłudze komendy z UI — agentki nie mają tych komend.
- `FrameSink` — odbiorca ramek (`TerminalFrame::Output { data_b64 }` / `Exit`); w powłoce to
  `tauri::ipc::Channel<TerminalFrame>`. Strumień nigdy nie trafia do zdarzeń `alfa://events`,
  magistrali ani logów (na magistrali tylko `ui-terminal.*` bez treści).
- `Programs` — profile `shell` (pwsh / PowerShell), `cmd`, `claude_login`, `codex_login` (programy
  z wykrycia CLI mostów). Logowanie do CLI wykonuje człowiek; Alfa nie czyta tokenów CLI.

Testy: `tests/terminal.rs` (szpieg: token z ekranu logowania i wpisane hasło poza zdarzeniami
i `Debug`), `crates/app-core/tests/computer.rs`.
