# platform-fake

Deterministyczna atrapa `SystemPort` (docs/PLAN.md §4.5) do testów innych modułów bez Windows.
`FakeFs` to wirtualny system plików w pamięci (model plikowy, katalogi wynikają ze ścieżek)
z dziennikiem cofnięć: `write_atomic`, `copy`, `move_path`, `delete_to_recycle_bin` zwracają
`OpReceipt` z `UndoToken`, a `undo` przywraca dokładnie poprzedni stan; `delete_permanent` jest
nieodwracalne. Deny-lista poświadczeń (`.claude`, `.codex`, `Cookies`…) działa jak w kontrakcie.
`FakeClipboard`, `FakeHotkeys` (`press`/`release` symulują zdarzenia), `FakeWindows`, `FakeProcesses`
i `FakeTray` rejestrują wywołania do asercji. `FakePlatform` składa wszystko w jeden `SystemPort`.
Testy właściwościowe (proptest) sprawdzają, że każda odwracalna operacja FS cofa się do migawki.
F3/2: `FakePipes` (potoki w pamięci z DACL, etykietą i ochroną pierwszej instancji + rejestr tożsamości procesów —
„proces o innym SID”), `FakeSurface` (skrypt zdarzeń okna), `FakeLauncher`, `FakePrivateDirs`, `FakeServiceHost`,
`FakeMmcss`, `FakeDisk` (`tests/kernel_ports.rs`).
F6/F4: `FakeDesktop` — wirtualny pulpit (okna z kolejnością Z, drzewa elementów UIA, kursor, wirtualny zegar,
fizyczne wejście, skrypty „Broker-UI na wierzch po N paczkach”, zawieszenie UIA, render do zrzutów); każdy skutek
w oknie trafia do `GuiRecord` z PID-em i obrazem (property w `tests/desktop.rs`: 0/200 w oknach chronionych).
`FakePty` — pseudokonsola w pamięci (wyjście skryptowane, echo, `exit`, `tree_killed`; `Debug` bez treści).
