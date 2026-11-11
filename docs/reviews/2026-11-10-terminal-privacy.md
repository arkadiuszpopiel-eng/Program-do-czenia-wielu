# Terminal — deterministyczna próba prywatności (#70)

**Aktualizacja10.11:** kwalifikacja tego zwykłego testu app-core jako chronionego core-* była zbyt szeroka. Dokładna łatka jest przygotowana wbf9ffe05 w ramach zlecenia napraw; bieżącą bazę weryfikuje nowyCI. Bez zmian polityk/tagów prywatności; przegląd przed merge nadal wymagany. [Aktualny raport](2026-11-10-defect-closure.md).

Baza: `b4e2dbcdfd64e5379b1499f870014aa39d8a011a`, PR24, Cloud Linux.

## Problem i zakres

CI180 nie przeszedł na Windows: `terminal_input_only_by_command_and_never_in_events`
w `app-core/tests/computer.rs` odrzucił obecność ciągu `4417`. Testowe wejście brzmi
`Lista zakupow mleko 4417`. Cztery cyfry mogą występować w legalnym identyfikatorze procesu;
same nie dowodzą wycieku treści. Nie zapisano pełnego zdarzenia z porażki CI180, więc nie
przypisujemy tamtemu zdarzeniu konkretnego PID ani innego pola.

Nowy test modułu terminala wymusza PID4417. Rzeczywisty `TerminalManager` z `FakePty`
przekazuje wejście do testowego `RecordingSink`, ale na magistrali są wyłącznie pola:
`terminal.opened` — pid/profile/terminal, `terminal.closed` — terminal,
`terminal.exited` — code/terminal. Nie ma pełnego wejścia ani banera `PS> `.
Nie zmieniono implementacji terminala lub dostępu agentek.

## Wyniki

- `cargo +1.94.1 test --locked -p ui-terminal-contract -p ui-terminal-impl -p ui-terminal-fake --all-features -j2`:
  **9 zaliczonych, 0 błędów, 0 pominiętych**, 10 wyników celów.
- Clippy tych trzech crate'ów, wszystkie cele i funkcje, `-D warnings`: **kod0**.
- Log testów SHA256: `7f6bf865c4ee90c6343fdd31d55eaa0feac82f7b8d0d0a4b25b7908331d2a925`.
- Log Clippy SHA256: `0a976b102f423a2b7558697659d85d96a07a486706417f3da5633d1e8052f7b4`.
- Pierwsze wersje nowego testu nie kompilowały serializacji `Event` i używały względnej
  ścieżki programu; poprawiono adapter testowy, zachowując logi tych niepowodzeń.

To atrapa pseudokonsoli na Linux, nie odbiór prawdziwego ConPTY lub terminala logowania na Windows.

## Historia wcześniejszej kwalifikacji testu

[Łatka](patches/terminal-privacy-marker.patch) sprawdza pełny tekst i jego zapis base64,
zamiast przypadkowej zbieżności czterech cyfr. SHA256:
`636b5802fba693d6ffe951abab4ad538b094dc539bfcd15a80b08a92636e1e82`.

Osobny worktree przeglądu: test Core `computer` **5/5**, Clippy tego celu **kod0**.
Log testu SHA256 `e407de71faf4184d690e7f4328e177b5f84e51dae1d2f7eaae2d019527412a21`;
Clippy `4845b7fed88a33b66531b478a8ab4336a4d1cab6ba41e6cee2b189ee4c57696a`.

W chwili pierwotnego wpisu łatka nie była zastosowana w gałęzi produktu i została odłożona do przeglądu.
Wcześniejsze zgody na importy Brokera i SQLCipher nie obejmują tego testu. Nawet ewentualny
sukces kolejnego CI bez łatki nie zamknie deterministycznie wykazanego problemu predykatu testu.

Surowe logi, komendy i kody wyjścia: `/workspace/scratch/coding-workflow`.
