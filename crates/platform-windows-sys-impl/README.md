# platform-windows-sys-impl

Sygnały systemowe i obserwacja katalogów dla Windows (docs/modules/platform-windows/SPEC.md, PLAN §3.4, §9.3, §9.5, §10)
— implementacja portów `platform-contract`. Nowy crate kategorii `platform-windows-*-impl` (limit 8 000 linii na crate);
windows-rs w tej samej wersji (`deny.toml`: `wrappers` dla `windows`).

| Port | Implementacja |
|---|---|
| `IdlePort` | `GetLastInputInfo` + `GetTickCount64` (różnica modulo 2³² — `dwTime` przekręca się co 49,7 dnia) |
| `PowerPort` | `GetSystemPowerStatus` → `PowerSnapshot::from_system_power_status` (zasilacz/bateria, poziom, oszczędzanie, czas) |
| `FullscreenPort` | `SHQueryUserNotificationState` (`QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`) + okno pierwszego planu pokrywające monitor bez ramki (pulpit `Progman`/`WorkerW`, pasek zadań i okna tego procesu wykluczone) |
| `SessionPort` | `WTSQuerySessionInformationW(WTSSessionInfoEx)`: `SessionFlags` (blokada) i `SessionState` (rozłączenie) |
| `SystemSignalsPort` (`WinSignals::start`) | wątek z oknem `HWND_MESSAGE`: `WTSRegisterSessionNotification`, `RegisterPowerSettingNotification` (zasilacz, poziom, oszczędzanie) budzą próbkę od razu; `SetTimer` (1 s) próbkuje bezczynność i pełny ekran; histereza i filtr zmian — `SignalMonitor` z kontraktu |
| `DirWatchPort` (`WinDirWatch`) | wątek na obserwację: `ReadDirectoryChangesW` z `OVERLAPPED`, bufor 64 KiB, stop = zdarzenie + `CancelIoEx`; przepełnienie (0 B / `ERROR_NOTIFY_ENUM_DIR`) i przeniesienie podkatalogu → pełne przeskanowanie (`std::fs`, bez dowiązań/junction i katalogów z deny-listy); deny-lista na ścieżce surowej i kanonicznej przed otwarciem katalogu; debounce i semantyka zmian — `WatchSet` z kontraktu |

Parsowanie `FILE_NOTIFY_INFORMATION` (`parse_notify_buffer`) i skan (`scan_dir`) są przenośne i testowane na Linuksie.
`unsafe` tylko w modułach FFI z `#[allow(unsafe_code)]`; każdy blok ma `// SAFETY:`. Poza Windows zapytania zwracają
`Unsupported`, a monitor i obserwacja się nie uruchamiają (polityka obserwacji jest sprawdzana wcześniej).
Testy Windows: `tests/sys_windows.rs` (CI bez pulpitu: zapytania, monitor, katalog tymczasowy, junction, przepełnienie
małego bufora; `#[ignore]`: blokada `Win+L`, gra pełnoekranowa).
