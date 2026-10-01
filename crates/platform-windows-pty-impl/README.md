# platform-windows-pty-impl

Pseudokonsola ConPTY (`WinPty`, port `PseudoConsolePort` z `platform-contract`) dla wbudowanego terminala
`ui-terminal` (F4; docs/modules/ui-terminal/SPEC.md). `CreatePseudoConsole` + potoki anonimowe, proces wstrzymany →
Job Object (`KILL_ON_JOB_CLOSE`) → wznowienie; zamknięcie zabija drzewo i zamyka pseudokonsolę (osobny wątek).
Środowisko jawne (blok UTF-16), wiersz poleceń wg reguł MSVCRT (`cmdline.rs`, testowane na każdej platformie).
Bez logowania i analizy strumienia. Testy Windows CI: `tests/conpty_windows.rs`.
