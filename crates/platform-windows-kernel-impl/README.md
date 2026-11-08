# platform-windows-kernel-impl

Porty Jądra bezpieczeństwa z `platform-contract` dla Windows (docs/modules/platform-windows/SPEC.md,
PLAN §8.1–8.4). Wydzielone z `platform-windows-impl` (limit 8 000 linii na crate; przegląd
bezpieczeństwa 2026-10, `docs/reviews/2026-10-security-review-1.md`). Drugi crate z windows-rs
(ta sama wersja; `deny.toml` → `wrappers` dla `windows`). Używa go wyłącznie korzeń kompozycji
procesów Jądra (`app-safety`).

| Port | Implementacja |
|---|---|
| `SecurePipePort` (`WinKernel`, `win_pipe.rs`) | `CreateNamedPipeW` z deskryptorem z SDDL (`PipeSecurity::sddl`), `PIPE_REJECT_REMOTE_CLIENTS`, `FILE_FLAG_FIRST_PIPE_INSTANCE`; klient z minimalnymi prawami i `SECURITY_IDENTIFICATION`. |
| `ProcessIdentityPort`, `PrivateDirPort`, `MmcssPort`, `DiskPort` (`win_sec.rs`) | obraz, SID tokenu, integralność, sesja; katalog prywatny z chronionym DACL (istniejący: odmowa dla dowiązania/junction i obcego właściciela); MMCSS RAII; wolne miejsce. |
| `ServiceHostPort`, `WinSessionLauncher` (`win_launch.rs`) | host usługi (`StartServiceCtrlDispatcherW`); Broker-UI w sesji konsoli z etykietą High, bilet przez stdin. |
| `ApprovalSurfacePort` (`WinApprovalSurface`, `surface/`) | okno Win32 bez WebView, hooki LL i `GetCurrentInputMessageSource` (wejście wstrzyknięte = odmowa), test zasłonięcia. |

`unsafe` tylko w modułach FFI z `#[allow(unsafe_code)]`; każdy blok ma `// SAFETY:`. Poza Windows
crate się kompiluje, porty zwracają `PlatformError::Unsupported`. Testy Windows CI portów —
`app-safety/tests/windows_ports.rs`; tu `tests/kernel_windows.rs` (`SendInput` w okno, `#[ignore]`).
