# windows (windows-rs) — tylko w `platform-windows-impl`

- Wersja: **`windows` 0.62.2** + **`windows-core` 0.62.2** (ta sama rodzina; `windows-core` wprost, bo
  makro `#[implement]` generuje ścieżki `::windows_core::…`). Przypięte w
  `crates/platform-windows-impl/Cargo.toml` (`[target.'cfg(windows)'.dependencies]`) i w `Cargo.lock`.
  `sysinfo` 0.38 (tylko poza Windows w `device-profile-impl`) wymaga `windows >=0.62,<0.63` — jedna wersja.
- Docs: https://microsoft.github.io/windows-docs-rs/ · źródło: `~/.cargo/registry/src/*/windows-0.62.2`.
- Zweryfikowano: `cargo clippy --target x86_64-pc-windows-msvc --all-targets -D warnings` 2026-09-30
  (bez linkowania; testy Windows wykonuje CI `windows-latest`).

## Feature'y (minimalny zestaw)
`Win32_Foundation`, `Win32_Security(_Authorization)`, `Win32_Storage_FileSystem`, `Win32_System_Com`,
`Win32_System_Com_StructuredStorage` + `Win32_System_Variant` (razem włączają `Display`/`Drop` dla
`PROPVARIANT`), `Win32_System_DataExchange`, `Win32_System_Memory`, `Win32_System_JobObjects`,
`Win32_System_Threading`, `Win32_System_Diagnostics_ToolHelp`, `Win32_System_LibraryLoader`,
`Win32_System_Power`, `Win32_System_Registry`, `Win32_System_SystemInformation`,
`Win32_UI_WindowsAndMessaging`, `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_Shell`,
`Win32_UI_Shell_PropertiesSystem`, `Win32_UI_HiDpi`, `Win32_Graphics_Gdi`, `Win32_Graphics_Dwm`,
`Win32_Graphics_Dxgi`, `Win32_Graphics_DXCore`, `Win32_Media_Audio`, `Win32_Devices_FunctionDiscovery`;
F3/2 (porty Jądra): `Win32_System_IO` (`ReadFile`/`WriteFile`/`ConnectNamedPipe` wymagają typu `OVERLAPPED`),
`Win32_System_Pipes`, `Win32_System_RemoteDesktop` (`WTS*`), `Win32_System_Services`, `Win32_UI_Input`
(`GetCurrentInputMessageSource`).

## Konwencje API 0.62 (różnice względem starszej wiedzy)
- Funkcje zwracające `BOOL` z `SetLastError` mają postać `-> windows_core::Result<()>`
  (`CloseHandle`, `OpenClipboard`, `RegisterHotKey`, `MoveFileExW`, `SetInformationJobObject`…).
  Czyste zapytania zostają przy `BOOL` (`IsWindowVisible`, `SetForegroundWindow`, `ShowWindowAsync`,
  `IsIconic`) — `BOOL` jest `#[must_use]`, sprawdzaj `.as_bool()`.
- `BOOL` jest w `windows::core::BOOL`; `HANDLE`/`HWND`/`HGLOBAL` to `struct X(pub *mut c_void)` —
  **nie `Send`** (własny `OwnedHandle` z `unsafe impl Send + Sync` w `win.rs`). `is_invalid()` sprawdza 0/−1.
- Parametry opcjonalne to `Option<T>`: `OpenClipboard(Some(hwnd))`, `RegisterHotKey(None, id, …)`,
  `CreateWindowExW(…, Some(HWND_MESSAGE), None, None, None)`, `QueryInformationJobObject(Some(job), …)`.
- Napisy: `w!("…")` (stałe), `PCWSTR(buf.as_ptr())` dla bufora z `OsStr::encode_wide().chain(Some(0))`;
  `PWSTR::to_string()` jest `unsafe`; pamięć z powłoki zwalnia `CoTaskMemFree(Some(p.0 as _))`.
- Rejestr: `RegGetValueW(HKEY_LOCAL_MACHINE, w!(…), w!(…), RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY, None,
  Some(buf), Some(&mut bytes)) -> WIN32_ERROR` (`.is_ok()`; najpierw wywołanie z `None` po rozmiar).
- COM: `CoCreateInstance::<_, IFileOperation>(&FileOperation, None, CLSCTX_ALL)`; fabryki bez
  `CoInitialize`: `CreateDXGIFactory1::<IDXGIFactory1>()`, `DXCoreCreateAdapterFactory::<IDXCoreAdapterFactory>()`.
- Implementacja interfejsu COM: `#[windows::core::implement(IFileOperationProgressSink)] struct Sink {..}`
  + `impl IFileOperationProgressSink_Impl for Sink_Impl { … }` (wszystkie metody; argumenty
  interfejsów jako `windows::core::Ref<IShellItem>` → `.as_ref() -> Option<&IShellItem>`); obiekt:
  `let sink: IFileOperationProgressSink = Sink { .. }.into();`.

## Używane wywołania (wg modułu)
| Moduł | Wywołania |
|---|---|
| `fs/native.rs` | `MoveFileExW(src, dst, MOVEFILE_COPY_ALLOWED \| MOVEFILE_WRITE_THROUGH)` (bez REPLACE = atomowe „nie nadpisuj”), `SHGetKnownFolderPath(&FOLDERID_*, KF_FLAG_DEFAULT, None)` |
| `fs/recycle.rs` (STA) | `IFileOperation::{SetOperationFlags, DeleteItem, PerformOperations, GetAnyOperationsAborted}`, flagi `FOF_ALLOWUNDO \| FOFX_RECYCLEONDELETE \| FOF_NOCONFIRMATION \| FOF_NOERRORUI \| FOF_SILENT \| FOF_WANTNUKEWARNING \| FOFX_EARLYFAILURE`, `SHCreateItemFromParsingName` (bez prefiksu `\\?\`), sink `PostDeleteItem(.., psiNewlyCreated)` → `IShellItem::GetDisplayName(SIGDN_FILESYSPATH)` = ścieżka `$R…` w Koszu (obok `$I…` z metadanymi) |
| `process/win.rs` | `CreateJobObjectW`, `SetInformationJobObject(JobObjectExtendedLimitInformation / JobObjectCpuRateControlInformation)`, `CreateProcessW` / `CreateProcessAsUserW` z `CREATE_SUSPENDED`, `AssignProcessToJobObject`, `ResumeThread`, `TerminateJobObject`, `QueryInformationJobObject(JobObjectBasicAccountingInformation)`, `OpenProcessToken`, `DuplicateTokenEx`, `ConvertStringSidToSidW(w!("S-1-16-4096"))`, `SetTokenInformation(TokenIntegrityLevel)`, `GetTokenInformation(TokenElevation)`, `CreateToolhelp32Snapshot` + `Process32FirstW/NextW` |
| `clipboard/win.rs` | okno `CreateWindowExW(w!("STATIC"), parent = HWND_MESSAGE)`, `OpenClipboard` (ponawiane), `EmptyClipboard`, `GetClipboardData`, `SetClipboardData(fmt, Some(HANDLE(hglobal.0)))`, `GlobalAlloc(GMEM_MOVEABLE)`/`GlobalLock`/`GlobalSize`, `DragQueryFileW`, `RegisterClipboardFormatW` (`PNG`, `CanIncludeInClipboardHistory`, `CanUploadToCloudClipboard`, `ExcludeClipboardContentFromMonitorProcessing`, `Preferred DropEffect`). `CF_*` jako stałe liczbowe (bez feature `Win32_System_Ole`). |
| `window/win.rs` | `EnumWindows(Some(cb), LPARAM(&mut vec as *mut _ as isize))`, `DwmGetWindowAttribute(DWMWA_CLOAKED / DWMWA_EXTENDED_FRAME_BOUNDS)`, `MonitorFromWindow` + `GetMonitorInfoW`, `GetDpiForWindow`, `QueryFullProcessImageNameW`, `SetForegroundWindow` (+ `AttachThreadInput` przy blokadzie fokusu), `ShowWindowAsync` |
| `hotkey/thread.rs` | `PeekMessageW(PM_NOREMOVE)` (utworzenie kolejki), `SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), GetModuleHandleW(None).map(HINSTANCE::from), 0)`, `GetMessageW`, `PostThreadMessageW(WM_APP+1 / WM_QUIT)`, `RegisterHotKey(None, id, HOT_KEY_MODIFIERS(m \| MOD_NOREPEAT), vk)` (błąd 1409 = zajęty), `SetTimer(None, 0, 15, None)` (licznik wątku → `WM_TIMER`), `GetAsyncKeyState` |
| `hardware/win.rs` | `GetLogicalProcessorInformation` (`RelationProcessorCore`, `RelationCache` L3), `GetPhysicallyInstalledSystemMemory`, `IDXGIFactory1::EnumAdapters1` + `GetDesc1` (koniec: `DXGI_ERROR_NOT_FOUND`), DXCore `CreateAdapterList(&[GENERIC_ML])` (GUID `b71b0d41-…-0250b7d3a988` zdefiniowany lokalnie — brak w 0.62), `GetSystemPowerStatus`, MMDevice (`IMMDeviceEnumerator::EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)`, `OpenPropertyStore(STGM_READ)`, `GetValue(&PKEY_Device_FriendlyName)` → `PROPVARIANT::to_string()`) na wątku MTA |

| `kernel/win_pipe.rs` | `ConvertStringSecurityDescriptorToSecurityDescriptorW(SDDL_REVISION_1)` → `SECURITY_ATTRIBUTES`, `CreateNamedPipeW(PIPE_ACCESS_DUPLEX \| FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_TYPE_BYTE \| PIPE_READMODE_BYTE \| PIPE_WAIT \| PIPE_REJECT_REMOTE_CLIENTS, …)` (zwraca `HANDLE`, błąd = `INVALID_HANDLE_VALUE`), `ConnectNamedPipe(h, None)` (`ERROR_PIPE_CONNECTED` = sukces), `GetNamedPipeClientProcessId`/`GetNamedPipeServerProcessId`, klient `CreateFileW(…, 0x12008B, FILE_SHARE_NONE, None, OPEN_EXISTING, SECURITY_SQOS_PRESENT \| SECURITY_IDENTIFICATION, None)` + `WaitNamedPipeW` przy `ERROR_PIPE_BUSY`, `ReadFile` → `ERROR_BROKEN_PIPE` = koniec strumienia |
| `kernel/win_sec.rs` | `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`, `OpenProcessToken(TOKEN_QUERY)`, `GetTokenInformation(TokenUser / TokenIntegrityLevel / TokenSessionId)` (bufor `Vec<u64>` — wyrównanie), `ConvertSidToStringSidW` + `LocalFree`, `GetSidSubAuthorityCount`/`GetSidSubAuthority` (RID), `CreateDirectoryW` z deskryptorem / `GetSecurityDescriptorDacl` + `SetNamedSecurityInfoW(SE_FILE_OBJECT, DACL \| PROTECTED_DACL)`, `AvSetMmThreadCharacteristicsW` / `AvRevertMmThreadCharacteristics`, `GetDiskFreeSpaceExW` |
| `kernel/win_launch.rs` | `WTSGetActiveConsoleSessionId`, `WTSQueryUserToken` (SeTcbPrivilege), `DuplicateTokenEx(TokenPrimary)`, `SetTokenInformation(TokenIntegrityLevel, S-1-16-12288)`, `CreatePipe` + `SetHandleInformation(HANDLE_FLAG_INHERIT, 0)`, `CreateProcessAsUserW(lpDesktop = winsta0\\default, STARTF_USESTDHANDLES)`; usługa: `StartServiceCtrlDispatcherW`, `RegisterServiceCtrlHandlerExW`, `SetServiceStatus` |
| `kernel/surface/*` | `RegisterClassExW`, `CreateWindowExW(WS_EX_TOPMOST \| WS_EX_DLGMODALFRAME)`, STATIC `0x80` (`SS_NOPREFIX` bez feature `Win32_UI_Controls`), BUTTON `BS_PUSHBUTTON` (bez `BS_DEFPUSHBUTTON`), `IsDialogMessageW`, `WM_CTLCOLORSTATIC`, `FlashWindowEx`, `ShowWindow(SW_SHOWNOACTIVATE)`, `SetWindowsHookExW(WH_KEYBOARD_LL / WH_MOUSE_LL)`, `GetCurrentInputMessageSource`, `GetWindow(GW_HWNDPREV)` + `DwmGetWindowAttribute(DWMWA_CLOAKED)` (zasłonięcie) |

## Pułapki
- `#[implement]` bez zależności `windows-core` → „could not find `windows_core`”.
- `IFileOperation` działa tylko w STA — nigdy na wątku tokio; `run_in_apartment(Apartment::Sta, ..)`.
- Callback `WH_KEYBOARD_LL` musi wracać natychmiast (limit `LowLevelHooksTimeout`), a wątek hooka
  musi pompować komunikaty; hook nie widzi klawiszy przy oknie administratora na wierzchu (UIPI).
- `FOF_NOCONFIRMATION` bez `FOF_WANTNUKEWARNING` po cichu usuwa trwale elementy za duże na Kosz.
- Synchroniczny uchwyt potoku serializuje operacje na obiekcie pliku — połączenie czyta i pisze jeden wątek
  (protokół żądanie → odpowiedź); `DisconnectNamedPipe` gubi nieodczytane dane, więc serwer tylko zamyka uchwyt.
- Podniesienie etykiety integralności tokenu i `WTSQueryUserToken` wymagają `SeTcbPrivilege` (usługa).
