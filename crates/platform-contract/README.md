# platform-contract

Neutralny (bez `windows-rs`) kontrakt dostępu do systemu dla modułu `platform-windows`
(docs/PLAN.md §1.2, §3.2; docs/modules/platform-windows/SPEC.md). `SystemPort` to suma sub-traitów:
`FsPort`, `ProcessPort`, `ClipboardPort`, `WindowPort`, `HotkeyPort`, `TrayPort` — każdy moduł
korzysta z systemu wyłącznie przez nie. Każda operacja mutująca ma flagę `reversible`
(`FsOperation::is_reversible`, `OpReceipt`) i, jeśli odwracalna, `UndoToken` dla `FsPort::undo`.
Błędy: `PlatformError` (m.in. `Denylisted` dla ścieżek poświadczeń, `NotReversible`,
`HotkeyConflict` — skrót zajęty przez inną aplikację). `HardwarePort` (poza sumą `SystemPort`,
dla `device-profile`) daje surowe dane o sprzęcie: CPU, RAM, GPU, NPU, zasilanie, audio, ziarno
identyfikatora maszyny. `Hotkey::validate` egzekwuje regułę AltGr (zakaz `Ctrl+Alt(+Shift)` + a, c, e, l, n, o, s, x, z)
i rezerwuje kill-switch `Ctrl+Shift+F12`. Implementacje: `platform-fake` (Linux/CI), `platform-windows-impl` (F1).

F3/2 — porty Jądra bezpieczeństwa (poza `SystemPort`): `SecurePipePort` + `PipeSecurity` (SDDL z chronionym DACL
na SID-y, klient bez prawa tworzenia instancji, etykieta integralności), `ProcessIdentityPort` + `PeerRequirement`
(`Sid`, `IntegrityLevel`, `SignatureStatus`, `CodeSignaturePort`/`UnverifiedSignatures`), `ApprovalSurfacePort`
(`SurfaceView` z walidacją i układem, `SurfaceEvent`), rozpoznawanie wstrzyknięć (`HookOrigin`, `MessageOrigin`,
`input_is_injected` — fail-closed), `ServiceHostPort`/`StopSignal`, `SessionLauncherPort` (`LaunchIntegrity`),
`PrivateDirPort` (`private_dir_sddl`), `MmcssPort`/`ThreadBoost` (RAII, `!Send`), `DiskPort`.

F6/F4 — computer use i terminal (poza `SystemPort`): `TargetGuard` + `GuiError` (okna Alfy/Brokera/helpera
i procesów nieznanych nigdy nie są celem — fail-closed), `DesktopPort` (okna v2, monitory), `UiaPort`
(`UiaTree`, `UiaQuery`, `UiaAction::check` — zakaz wpisywania w pole hasła, `ElementRef` = okno + `RuntimeId`),
`InputPort` + `InputBackend` + `execute_input` (paczki atomowe, cel/strażnik/UIPI/fizyczne wejście przed każdą
paczką, `KeyChord::system_scope` — zakaz skrótów systemowych), `ScreenCapturePort` + `mask_plan`/`finish_capture`
(maskowanie, `RgbaImage`, PNG z wymiennym zlib), `PseudoConsolePort`/`PtySession` (ConPTY).

Sygnały systemowe i obserwacja katalogów (poza `SystemPort`; Windows: `platform-windows-sys-impl`): `IdlePort` +
`IdleTracker` (histereza: wejście po 5 min, wyjście po ≥ 1 s aktywności), `PowerPort` + `PowerSnapshot`
(dekodowanie `SYSTEM_POWER_STATUS`, filtr zmian), `FullscreenPort` + `FullscreenProbe`/`GameModeTracker` (`QUNS_*`,
okno pełnoekranowe; wyjście z trybu gry po 10 s), `SessionPort` + `SessionState` (blokada/rozłączenie), zbiorczo
`SystemSignalsPort` + `SignalMonitor` (`SystemSignals`, `SignalEvent`, `platform.idle.*`/`power`/`fullscreen`/
`session.*`); `DirWatchPort` + `WatchSet`/`WatchCore` (debounce z semantyką istnienia, pary przemianowań, pełne
przeskanowanie po przepełnieniu) + `WatchPolicy` (deny-lista surowa i kanoniczna — `BASELINE_DENY_SEGMENTS` ⊇
segmenty Jądra, limity, pliki tymczasowe, wzorce).
