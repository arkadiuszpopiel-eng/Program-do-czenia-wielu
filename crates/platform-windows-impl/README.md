# platform-windows-impl

Implementacja `SystemPort` + `HardwarePort` z `platform-contract` dla Windows
(docs/modules/platform-windows/SPEC.md, PLAN §3.2, §7.3, §8.6). Jedyny crate workspace z windows-rs
(`windows` 0.62.2 + `windows-core` dla `#[implement]`; skrót API: `docs/vendor/windows.md`).
`unsafe` tylko w modułach FFI (`win.rs`, `*/win.rs`, `fs/native.rs`, `fs/recycle.rs`,
`hotkey/thread.rs`) z `#[allow(unsafe_code)]`; każdy blok ma `// SAFETY:` (clippy
`undocumented_unsafe_blocks = deny`). Poza Windows crate się kompiluje: FS działa przenośnie
(bez Kosza), reszta zwraca `PlatformError::Unsupported`.

| Port | Implementacja |
|---|---|
| `FsPort` (`WinFs`) | deny-lista przed każdą operacją (kontrakt + dodatkowe nazwy/prefiksy z `FsConfig`; postaci: surowa, `%ZMIENNE%`, leksykalna z `\\?\`/wielkością liter/końcowymi kropkami/ADS, kanoniczna przez `canonicalize` = `GetFinalPathNameByHandleW` — dowiązania, junctions, nazwy 8.3); zapis atomowy (tmp + `rename`) z kopią do cofnięcia; kopiowanie/przenoszenie bez nadpisywania (`MoveFileExW` bez REPLACE); Kosz przez `IFileOperation` (STA) z odbiorem położenia `$R…` w sinku → cofnięcie; trwałe usuwanie = pokwitowanie nieodwracalne. Cofnięcie odmawia, jeśli plik zmieniono po operacji. |
| `ProcessPort` (`WinProcesses`) | proces wstrzymany → własny Job Object (`KILL_ON_JOB_CLOSE`, `DIE_ON_UNHANDLED_EXCEPTION`, limity pamięci/CPU/affinity, `JobLimits::emulate` dla baseline) → wznowienie; `kill_tree` = `TerminateJobObject` (< 200 ms); integralność Low przez token S-1-16-4096; AppContainer → `Unsupported` (F3). Tylko ścieżki bezwzględne, bez `.bat/.cmd`. |
| `ClipboardPort` (`WinClipboard`) | tekst, pliki (`CF_HDROP`), obraz (`PNG`; odczyt także `CF_DIB/V5` → PNG); treść z `ExcludeClipboardContentFromMonitorProcessing` nie jest czytana; zapisy z `CanIncludeInClipboardHistory=0` i `CanUploadToCloudClipboard=0`; `set_sensitive` dodaje wykluczenie z monitorów. |
| `WindowPort` (`WinWindows`) | `list_detailed` (tytuł, proces, PID, prostokąt DWM, monitor, DPI, pełny ekran), fokus, minimalizacja, przywracanie; okna procesów Alfy/Brokera i bieżącego procesu chronione (`WindowGuard`). |
| `HotkeyPort` (`WinHotkeys`) | `RegisterHotKey` (`MOD_NOREPEAT`) na dedykowanym wątku z pętlą komunikatów + `WH_KEYBOARD_LL` do puszczenia klawisza (PTT) + zapasowe `GetAsyncKeyState` co 15 ms (okno admina na wierzchu); konflikt → `HotkeyConflict`; `register_kill_switch()` tylko dla watchdoga/Brokera; `wait_events` dla niskiej latencji. |
| `TrayPort` (`TrayAdapter`) | adapter: stan i menu w pamięci, rysuje backend powłoki Tauri (`TrayBackend`) — bez drugiej ikony `Shell_NotifyIconW`. |
| `HardwarePort` (`WinHardware`) | rejestr (model CPU, wersja systemu, `MachineGuid`), `GetLogicalProcessorInformation` (rdzenie, L3), `GetPhysicallyInstalledSystemMemory`, DXGI `EnumAdapters1`, DXCore (NPU), `GetSystemPowerStatus`, MMDevice (wątek MTA). |

Wątki: skróty/hook — własny wątek komunikatów; `IFileOperation` — krótkotrwały wątek STA;
MMDevice — krótkotrwały wątek MTA; schowek i okna — wątek wywołującego (bez COM). Nigdy COM na
wątku wywołującego (może to być wątek audio RT). Testy: `tests/fs_port.rs` (każdy OS, w tym
proptest LIFO), `tests/windows_system.rs` (Windows; skróty/schowek/Kosz `#[ignore]` — pulpit).
