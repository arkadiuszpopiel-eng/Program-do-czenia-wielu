# platform-windows — SPEC (szkic v0)

## Cel
Jedyny crate z windows-rs/COM, zamknięty za traitem `SystemPort`: pliki, procesy, schowek, okna, zasobnik, skróty globalne, hook klawiatury (PTT), ścieżki i ACL. Wszystkie moduły korzystają z systemu **wyłącznie** przez ten kontrakt (PLAN §1.2, §3.2). UIA i SendInput dochodzą w v1.5/v2.

## Fala i priorytet
F6 (zrobione: `platform-windows-gui-impl`, `platform-windows-pty-impl`): v2 — okna v2, UIA, SendInput, zrzuty z maskowaniem, ConPTY (F4). F0: `SystemPort`-contract + fake (pkt 2 w §4.5a). F1 (zrobione: `platform-windows-impl`): v1 (fs, procesy, schowek, okna, zasobnik, skróty globalne + `WH_KEYBOARD_LL`; bez UIA/SendInput). F5: v1.5 (SendInput tekstu, UIA `TextPattern` odczyt). F6: v2 (UIA, SendInput, zrzuty). P0.

## Kontrakt (stan F1 — źródło prawdy: `crates/platform-contract`)
```rust
pub trait SystemPort: FsPort + ProcessPort + ClipboardPort + WindowPort + HotkeyPort + TrayPort {}
pub trait HardwarePort { fn os/cpu/memory_total_mb/gpus/npu/power_status/audio_endpoints/machine_seed } // osobno, dla device-profile
// FsPort: read, write_atomic → OpReceipt{op, reversible, undo}, copy, move_path, delete_to_recycle_bin,
//         delete_permanent (nieodwracalne), exists, list_dir, undo(UndoToken), known_folder
// ProcessPort: spawn(ProcessSpec{cmd, args, cwd, integrity, memory_limit_mb}) (Job Object), kill_tree,
//              status, foreground_is_elevated · HotkeyPort: register (validate: AltGr, kill-switch),
//              unregister, drain_events(HotkeyEvent{id, pressed}) · PlatformError::{…, HotkeyConflict}
```
Impl (`platform-windows-impl`) ponad kontrakt: `WinWindows::list_detailed/minimize/restore` (PID, prostokąt,
monitor, DPI; `WindowGuard` chroni procesy Alfy/Brokera), `WinProcesses::spawn_with_limits(JobLimits)`
(emulacja baseline), `tree_size`, `release`, `list`; `WinHotkeys::register_kill_switch/wait_events`;
`WinClipboard::set_sensitive`; `TrayAdapter` + `TrayBackend` (ikonę rysuje powłoka Tauri).
Zdarzenia: `platform.hotkey`, `platform.ptt`, `platform.clipboard.changed`, `platform.fs.changed`, `platform.device.changed`, `platform.fullscreen.changed`, `platform.session.locked` (publikacja przez jądro — F2).

### Porty Jądra bezpieczeństwa (F3, część 2 — poza sumą `SystemPort`)
Implementacja Windows żyje w crate'cie `platform-windows-kernel-impl` (wydzielony z `platform-windows-impl` przez limit rozmiaru; przegląd 2026-10; używa go tylko `app-safety`).
| Port (kontrakt) | Windows (`WinKernel`, `WinSessionLauncher`, `WinApprovalSurface`) | Atrapa |
|---|---|---|
| `SecurePipePort` + `PipeSecurity` (SDDL: chroniony DACL na SID-y, klienci bez `FILE_CREATE_PIPE_INSTANCE`, etykieta `NW`) | `CreateNamedPipeW` z deskryptorem, `PIPE_REJECT_REMOTE_CLIENTS`, `FILE_FLAG_FIRST_PIPE_INSTANCE`, zawsze wolna instancja; klient `SECURITY_SQOS_PRESENT \| SECURITY_IDENTIFICATION`; `peer_pid` = `GetNamedPipeClient/ServerProcessId` | `FakePipes` (DACL, etykieta, przejęcie nazwy, PID-y) |
| `ProcessIdentityPort` + `PeerRequirement::check` (konto, min. integralność, obraz, sesja, podpis) | `OpenProcess` → `QueryFullProcessImageNameW`, token: `TokenUser`, `TokenIntegrityLevel` (RID), `TokenSessionId` | `FakePipes` (rejestr tożsamości) |
| `CodeSignaturePort` | `UnverifiedSignatures` („niezweryfikowane” w dev; WinVerifyTrust + przypięcie certyfikatu po bramce #10) | — |
| `PrivateDirPort` (`private_dir_sddl`) | nowy: `CreateDirectoryW` z deskryptorem; istniejący: odmowa dla dowiązania/junction/pliku i dla właściciela spoza {konto usługi, SYSTEM, Administratorzy} (`GetNamedSecurityInfoW`; katalog założony zawczasu przez użytkownika zachowałby mu `WRITE_DAC`), potem `SetNamedSecurityInfoW` (chroniony DACL) | `FakePrivateDirs` |
| `SessionLauncherPort` (`LaunchIntegrity`) | `UserSessionHigh`: `WTSGetActiveConsoleSessionId` + `WTSQueryUserToken` → `DuplicateTokenEx` → etykieta High → `CreateProcessAsUserW` (`winsta0\default`, stdin = anonimowy potok); `AsCaller` — `app-safety::ChildLauncher` | `FakeLauncher` |
| `ServiceHostPort` + `StopSignal` | `StartServiceCtrlDispatcherW`, `RegisterServiceCtrlHandlerExW` (STOP/SHUTDOWN), `SetServiceStatus` | `FakeServiceHost` |
| `ApprovalSurfacePort` (`SurfaceView`, `SurfaceView::layout`, `SurfaceEvent`) | okno Win32 na własnym wątku (szczegóły: `broker-ui` SPEC) | `FakeSurface` (skrypt zdarzeń) |
| wejście wstrzyknięte: `HookOrigin` (flagi `LLKHF_INJECTED`, `LLKHF_LOWER_IL_INJECTED`, `LLMHF_*`), `MessageOrigin` (`IMO_*`), `input_is_injected` (fail-closed) | hooki LL na wątku okna + `GetCurrentInputMessageSource` | logika w kontrakcie |
| `MmcssPort` → `ThreadBoost` (RAII, `!Send`) | `AvSetMmThreadCharacteristicsW("Pro Audio")` / `AvRevertMmThreadCharacteristics` | `FakeMmcss` |
| `DiskPort::free_disk_space` | `GetDiskFreeSpaceExW` | `FakeDisk` |

### v2 (F6) — computer use i terminal (poza sumą `SystemPort`)
Implementacja Windows: `platform-windows-gui-impl` (`WinGui`) i `platform-windows-pty-impl` (`WinPty`) — wydzielone przez limit rozmiaru crate'a (`deny.toml`: `wrappers` dla `windows`); składa je korzeń kompozycji (`app-*`).
| Port (kontrakt) | Windows | Atrapa (`FakeDesktop`, `FakePty`) |
|---|---|---|
| `TargetGuard` + `GuiError` | strażnik celów: PID (bieżący proces, PID-y usług), obraz (`PROTECTED_IMAGES` = suma list Brokera i v1, alias 8.3), katalog instalacji; **obraz nieznany = chroniony** | ten sam typ |
| `DesktopPort` (okna v2) | `EnumWindows` (kolejność Z, bez okien ukrytych przez DWM), PID/obraz/`TokenElevation`, monitory (`EnumDisplayMonitors`, `GetDpiForMonitor`), fokus (`SetForegroundWindow` + `AttachThreadInput`), `SetWindowPos(SWP_ASYNCWINDOWPOS)` z korektą ramki DWM, `ShowWindowAsync`; strażnik tuż przed zmianą | okna w pamięci, kolejność Z |
| `UiaPort` | wątek COM MTA (`CUIAutomation8`), `IUIAutomation2` z limitami połączenia/transakcji, każde wywołanie z `recv_timeout` (5 s, drzewo 15 s); wiszący wątek porzucany i zastępowany (≤ 4 naraz, potem odmowa); `CacheRequest` (jedno wywołanie międzyprocesowe na węzeł), `ControlViewWalker`, odwołanie = HWND + `RuntimeId`; akcje przez wzorce; `TextPattern` tylko odczyt; wartość pola hasła nigdy nie wychodzi | drzewa elementów, symulacja zawieszenia |
| `InputPort` + `InputBackend` + `execute_input` | `SendInput` paczkami atomowymi (Unicode, VK z `EXTENDEDKEY`, mysz bezwzględnie na pulpicie wirtualnym); cel/strażnik/UIPI/fizyczne wejście przed każdą paczką (logika w kontrakcie); hook `WH_KEYBOARD_LL`+`WH_MOUSE_LL` na wątku z pętlą komunikatów — wejście niewstrzyknięte = użytkownik (brak hooka = odmowa wejścia) | wirtualny zegar, skrypty: fizyczne wejście, Broker-UI na wierzch |
| `ScreenCapturePort` + `mask_plan`/`finish_capture` | **BitBlt (`CAPTUREBLT`) / `PrintWindow(PW_RENDERFULLCONTENT)`** zamiast Windows.Graphics.Capture (synchronicznie, bez WinRT/D3D11 i żółtej ramki); maskowanie okien chronionych, `DEFAULT_MASKED_APPS` + żądania, pól haseł (UIA w budżecie 4 s, niesprawdzone okno = całe zamaskowane); czarna klatka wykrywana; skalowanie średnią z obszaru; PNG (`flate2`) | render kolorami, PNG bez kompresji |
| `PseudoConsolePort` | `CreatePseudoConsole` + potoki anonimowe, proces `CREATE_SUSPENDED` → Job Object `KILL_ON_JOB_CLOSE` → wznowienie; środowisko jawne (blok UTF-16), wiersz poleceń MSVCRT; `close` = `TerminateJobObject` + `ClosePseudoConsole` na osobnym wątku | `FakePty` |

Testy: `platform-fake/tests/desktop.rs` (property 0/200 skutków w oknach chronionych, przerwanie, maskowanie, limit czasu UIA), `platform-contract` (wykonawca wejścia, skróty systemowe, PNG/CRC/Adler, maski), Windows CI: `platform-windows-gui-impl/tests/gui_windows.rs` (pulpit `#[ignore]`: Notatnik — UIA, pisanie, zrzut), `platform-windows-pty-impl/tests/conpty_windows.rs` (wyjście, kod, zabicie drzewa).

### Sygnały systemowe i obserwacja katalogów (poza sumą `SystemPort`)
Implementacja Windows: `platform-windows-sys-impl` (`WinSignals`, `WinDirWatch`; nowy crate kategorii `platform-windows-*-impl`, `deny.toml`: `wrappers` dla `windows`). Reguły (histereza, filtr zmian, debounce, przeskanowanie, deny-lista) są w kontrakcie — Windows i atrapa różnią się tylko źródłem próbek/zmian.
| Port (kontrakt) | Windows | Atrapa |
|---|---|---|
| `IdlePort` + `IdleTracker` (wejście po `idle_after_ms` = 5 min; wyjście dopiero, gdy aktywność trwa ≥ `wake_confirm_ms` = 1 s w oknie 10 s — trącenie myszy nie przerywa zadań tła; błąd odczytu nie zmienia stanu) | `GetLastInputInfo` + `GetTickCount64` (różnica modulo 2³²); wejście syntetyczne Alfy też zeruje licznik (kierunek bezpieczny) | `FakeSignals::input`, wirtualny zegar |
| `PowerPort` + `PowerSnapshot::from_system_power_status` (zasilacz/bateria, poziom, oszczędzanie, czas; zmiana zgłaszana przy źródle, oszczędzaniu, kroku ≥ 5 pp albo przekroczeniu 20%) | `GetSystemPowerStatus`; powiadomienia `RegisterPowerSettingNotification` (`GUID_ACDC_POWER_SOURCE`, `GUID_BATTERY_PERCENTAGE_REMAINING`, `GUID_POWER_SAVING_STATUS`) budzą próbkę | `set_power` + `notify` |
| `FullscreenPort` + `FullscreenProbe::game_reason` + `GameModeTracker` (tryb gry od razu, wyjście po 10 s bez pełnego ekranu; brak odczytu = tryb trwa; `QUNS_NOT_PRESENT` to nie gra) | `SHQueryUserNotificationState` (`QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`) + okno pierwszego planu bez ramki pokrywające monitor (`Progman`/`WorkerW`/pasek zadań i okna Alfy wykluczone) | `set_fullscreen` |
| `SessionPort` + `SessionState::is_away` (zablokowana/rozłączona → głos wyciszony, computer use wstrzymany; `Unknown` nie zmienia stanu) | `WTSQuerySessionInformationW(WTSSessionInfoEx)` (`SessionFlags`, `SessionState`), powiadomienie `WM_WTSSESSION_CHANGE` | `set_session` |
| `SystemSignalsPort` + `SignalMonitor` (`SystemSignals`, `SignalEvent`, kolejka ≤ 256) | wątek z oknem `HWND_MESSAGE`: powiadomienia + `SetTimer` 1 s (bezczynność i pełny ekran nie mają powiadomień — jedyny wyjątek od „bez pollingu”, koszt ~0) | `FakeSignals` (próbki co `poll_ms` wirtualnego zegara; `wait_events` przesuwa zegar) |
| `DirWatchPort` + `WatchSet`/`WatchCore`/`WatchPolicy` (debounce 750 ms, najdłużej 30 s; semantyka istnienia: utworzony+usunięty = nic, usunięty+utworzony = zmieniony, tymczasowy→docelowy = utworzony, para nazw = `Renamed`; pliki tymczasowe i wzorce `*`/`?`; ≤ 16 obserwacji, ≤ 4096 plików pamiętanych na obserwację; deny-lista surowa i kanoniczna przed otwarciem katalogu, zmiany pod nią nigdy nie wychodzą; korzeń wolumenu z podkatalogami odrzucany) | `ReadDirectoryChangesW` z `OVERLAPPED` (wątek na obserwację, stop = zdarzenie + `CancelIoEx`), bufor 64 KiB; pierwsze żądanie przed skanem początkowym; 0 B / `ERROR_NOTIFY_ENUM_DIR` → `Rescanned{Overflow}`; dodany/przeniesiony podkatalog → `Rescanned{DirectoryMoved}`; skan bez dowiązań/junction | `FakeDirWatch` (wirtualny FS, `lose_events`, `overflow`, `move_dir`, `link`) |

Zdarzenia (publikuje `app-*`): `platform.idle.entered|exited`, `platform.power.changed`, `platform.fullscreen.changed`, `platform.session.locked|unlocked`, `platform.fs.changed|rescanned|watch_stopped` (ścieżki bez treści). Podpięcie w `app-*`: `SystemConditions { user_idle, game_mode }` dla schedulera, `IdleSource`/`HostConditions` Strażniczki, `ModeSource` (`model-residency::refresh_mode`), `FileWatchPort` wyzwalaczy (`replace_all` + `WatchEvent::new_file` → `TriggersModule::file_created`), blokada sesji → `voice-wake` (wyciszenie) i computer use (pauza).
Testy: `platform-fake/tests/signals.rs` (histereza, filtr zasilania, tryb gry, blokada, kolejka), `dir_watch.rs` (debounce, pobieranie, przemianowania, przepełnienie → dokładne różnice, deny-lista: 0 zdarzeń z `.ssh`/`.claude` także przez junction i przemianowanie, limity), `dir_watch_props.rs` (proptest 256 przypadków: strumień spójny i zbieżny ze stanem katalogu przy utracie zdarzeń); `platform-windows-sys-impl`: parser `FILE_NOTIFY_INFORMATION` i skan (Linux), spójność deny-listy z Jądrem, Windows CI (`tests/sys_windows.rs`: zapytania, monitor, katalog tymczasowy, junction, przepełnienie bufora 1 KiB; `#[ignore]`: `Win+L`, gra).

## Zależności
`platform-contract` (F1 bez magistrali — zdarzenia publikuje jądro). Zewnętrzne: `windows`/`windows-core` 0.62.2 (jedna wersja, `docs/vendor/windows.md`).

## Niezmienniki
- Poza tym crate'em brak `windows`/`windows-sys` w workspace (test CI grafu zależności).
- Skrót globalny naruszający regułę AltGr (`Ctrl+Alt(+Shift)` + a, c, e, l, n, o, s, x, z) jest odrzucany (PLAN §8.6; test CI).
- Usuwanie plików domyślnie do Kosza; `write_atomic` = tmp + rename.
- COM/UIA na dedykowanym wątku MTA; brak wywołań COM z wątku audio RT. F1: `IFileOperation` na krótkotrwałym wątku STA, MMDevice na wątku MTA, skróty i hook `WH_KEYBOARD_LL` na własnym wątku z pętlą komunikatów.
- Deny-lista sprawdzana na postaci surowej, po `%ZMIENNYCH%`, leksykalnej (`\\?\`, wielkość liter, końcowe kropki/spacje, ADS) i kanonicznej (dowiązania, junctions, 8.3); kopiowanie drzewa sprawdza każdy wpis.
- Usuwanie do Kosza, którego nie da się przenieść do Kosza, pyta użytkownika (`FOF_WANTNUKEWARNING`); zgoda = pokwitowanie nieodwracalne.
- Deny-lista ścieżek poświadczeń (`~/.claude`, `~/.codex`, profile przeglądarek, Credential Manager) egzekwowana tu jako ostatnia linia (nawet gdy `tools-fs` zawiedzie). Domyślne `extra_deny_names`/`extra_deny_prefixes` obejmują **całą** bazową deny-listę Jądra (`compliance`), bo tylko tu sprawdzana jest postać kanoniczna — dowiązanie/junction w profilu nie omija listy (przegląd 2026-10, SR-05; spójność: `tests/review.rs`).
- Hooki/PTT nie działają przy oknie administratora na pierwszym planie bez helpera `uiAccess` → `foreground_is_elevated()` i zdarzenie do UI (PLAN §7.3).

## Zdolności / uprawnienia
Port sam nie decyduje: wykonuje operacje z tokenem zdolności przekazanym przez wywołującego (od F3); w F1 (bez Brokera) tylko wywołania jądra i UI.

## Izolacja
`inproc`, `always` (część `process` — helper `uiAccess` w F6, osobny SPEC).

## Budżet zasobów
RAM ≤ 4 MB (+ obserwacje: ≤ 4096 ścieżek na obserwację, bufor 64 KiB); `spawn` ≤ 30 ms; reakcja hooka PTT ≤ 10 ms; watchery bez pollingu (wyjątek: monitor sygnałów próbkuje bezczynność i pełny ekran co 1 s — brak powiadomień systemowych).

## Konfiguracja (klucze TOML)
`[platform] recycle_bin = true`, `[platform.signals] idle_after = "5m"`, `wake_confirm = "1s"`, `game_exit_after = "10s"`, `poll = "1s"`, `[platform.watch] max_watches = 16`, `debounce = "750ms"`, `buffer = "64KiB"`, `[platform.hotkeys] kill_switch = "Ctrl+Shift+F12"`, `quick_ask = "Ctrl+Alt+Space"`, `ptt = "Space"`; `[platform.denylist_paths] = [...]` (kernel_policy).

## Wkład do UI
Brak własnego; dostarcza zasobnik/okna dla `shell-integration` i `ui-quick`, stan „okno admina na wierzchu" dla paska stanu.

## Testy akceptacyjne
- `ACC-F0-platform-windows-01`: kontrakt `SystemPort` przechodzi na `-fake` (wirtualny FS) i `-impl` (self-hosted runner).
- `ACC-F1-platform-windows-02`: reguła AltGr — zestaw zakazanych skrótów odrzucony 100%.
- `ACC-F1-platform-windows-03`: PTT — puszczenie klawisza zgłoszone ≤ 10 ms (hook), na runnerze desktop.
- `ACC-F1-platform-windows-04`: `kill_tree` zabija całe drzewo (Job Object) ≤ 50 ms.
- F3/2: logika portów Jądra na Linuksie (SDDL, SID, integralność, flagi wstrzyknięć, układ okna, `platform-fake/tests/kernel_ports.rs`); Windows CI: DACL egzekwowany (konto spoza listy = odmowa), pierwsza instancja, PID-y, tożsamość, katalog prywatny, MMCSS, dysk, host usługi (`app-safety/tests/windows_ports.rs`); pulpit/usługa `#[ignore]`: `SendInput` w okno = zawsze „wstrzyknięte”, uruchomienie z wysoką integralnością.

## Fake
`platform-windows-fake`: wirtualny system plików, schowek i lista okien w pamięci, skrypty zdarzeń (hotkey/PTT/hot-plug) z wirtualnym zegarem — testy na Linux/CI bez Windows.

## Otwarte pytania
- Obserwacja FS jest (`DirWatchPort`); schowek nadal bez `watch`, skróty bez subskrypcji zdarzeń — do SPEC v1 (zdarzenia na magistrali).
- Rozszerzyć kontrakt: `WindowInfo` o PID/prostokąt/DPI, `WindowPort` o minimalizację, `ProcessSpec` o limity CPU/affinity i stdio (dziś metody impl).
- Dziennik cofnięć FS jest w pamięci procesu — trwały dziennik to `undo-journal` (F3).
- Podział `SystemPort` na osobne crate'y kontraktowe per pod-port (ładowanie leniwe) — do ustalenia w SPEC v1.
- Snap Layouts/Mica przez Tauri (spike j) — czy własny pasek tytułu wymaga kodu tutaj.
- Rozmiar crate'a: porty Jądra (F3/2) wydzielone do `platform-windows-kernel-impl` (`deny.toml`: `wrappers` dla `windows`); `platform-windows-impl` ~6 400 linii `.rs` z testami, `platform-windows-kernel-impl` ~2 100.
- Authenticode (`WinVerifyTrust` + przypięcie wystawcy) i druga ścieżka tożsamości klienta (`ImpersonateNamedPipeClient`) — po bramce #10 (certyfikat).

## Utwardzenia po przeglądzie bezpieczeństwa #2 (2026-10, `docs/reviews/2026-10-security-review-2.md`)
- **P2-01 — strażnik celów:** `TargetGuard::check_window` sprawdza proces efektywny i wszystkie `ProcessLink` okna (okno, `GA_ROOT`, łańcuch `GW_OWNER`/`GA_ROOTOWNER`, treść UWP: proces okna `Windows.UI.Core.CoreWindow` w `ApplicationFrameWindow`; ramka bez treści = `UwpUnresolved`, chroniona). Przodkowie z migawki Toolhelp32 **przy każdym sprawdzeniu**: potomek bieżącego procesu albo PID-u z `pids` jest chroniony (proces WebView2 odtworzony po awarii, nowe okno przeglądarki, sidecary). Skutek uboczny (bezpieczny kierunek): okna procesów uruchomionych przez Alfę jako dzieci są poza zasięgiem agentek. `platform-windows-gui-impl/src/links.rs`; atrapa: `FakeWindow::{in_process, child_of, owned_by, uwp}`, `FakeDesktop::add_process` (`platform-fake/tests/review.rs`: 0/500 skutków).
- **P2-02 — zrzuty:** okna → klatka → okna ponownie (`capture_set_stable`); zmiana zbioru okien w obszarze = klatka ponowiona (`CAPTURE_ATTEMPTS = 3`), potem maska z sumy wyliczeń (`union_for_mask`) i okna zmienione maskowane w całości w obu położeniach (`unstable_masks`).
- **P2-03 — fokus:** `UiaPort::focused(window)` (UIA `GetFocusedElement` + `IsPassword`, domyślnie błąd = fokus nieznany) i `InputBackend::focused_field()` (domyślnie `Unknown`); `execute_input` odmawia paczek wpisujących treść, gdy fokus jest w polu hasła albo nieznany (`FocusedField`, `batch_writes_text`).
- **P2-04 — skróty globalne:** wątek skrótów zapisuje pochodzenie wciśnięć (`LLKHF_INJECTED`/`LLKHF_LOWER_IL_INJECTED`, `hotkey/origin.rs`); `WM_HOTKEY` z kombinacji wstrzykniętej jest ignorowany (`HotkeyPressOrigin::admits`), nieznane pochodzenie przyjmowane tylko przy oknie podniesionym na pierwszym planie (UIPI blokuje wtedy `SendInput`), kill-switch zawsze. Atrapa: `FakeHotkeys::press_injected`.
- **P2-05 — ConPTY:** lista atrybutów w buforze `Vec<usize>` (`attrs.rs`, zwalniana w `Drop` także na ścieżkach błędu); `write_input` nie trzyma zamka uchwytu podczas `WriteFile` (uchwyt `Arc`, porcje 4 KiB, przerwanie po zamknięciu) — `close()` nie czeka na zawieszony zapis.
- **P-07 (przegląd #1) — start Broker-UI:** potok bez dziedziczenia, dziedziczny tylko koniec do odczytu, `STARTUPINFOEXW` + `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` (jawna lista jednego uchwytu) — dziecko nie dziedziczy innych uchwytów usługi Brokera. Pozostaje okno, w którym inny wątek usługi tworzący proces z `bInheritHandles = TRUE` bez listy mógłby odziedziczyć ten koniec (dziś usługa takich nie tworzy). **Wymaga przeglądu człowieka** (`platform-windows-kernel-impl`, ścieżka Jądra).
