# platform-fake

Deterministyczna atrapa `SystemPort` (docs/PLAN.md §4.5) do testów innych modułów bez Windows.
`FakeFs` to wirtualny system plików w pamięci (model plikowy, katalogi wynikają ze ścieżek)
z dziennikiem cofnięć: `write_atomic`, `copy`, `move_path`, `delete_to_recycle_bin` zwracają
`OpReceipt` z `UndoToken`, a `undo` przywraca dokładnie poprzedni stan; `delete_permanent` jest
nieodwracalne. Deny-lista poświadczeń (`.claude`, `.codex`, `Cookies`…) działa jak w kontrakcie.
`FakeClipboard`, `FakeHotkeys` (`press`/`release` symulują zdarzenia), `FakeWindows`, `FakeProcesses`
i `FakeTray` rejestrują wywołania do asercji. `FakePlatform` składa wszystko w jeden `SystemPort`.
Testy właściwościowe (proptest) sprawdzają, że każda odwracalna operacja FS cofa się do migawki.
