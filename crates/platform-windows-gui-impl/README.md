# platform-windows-gui-impl

`platform-windows` v2 (F6; docs/modules/platform-windows/SPEC.md, PLAN §7): implementacja portów `DesktopPort`,
`UiaPort`, `InputPort`, `ScreenCapturePort` z `platform-contract` dla Windows (`WinGui`). Wydzielone z
`platform-windows-impl` (limit 8 000 linii na crate); windows-rs w tej samej wersji (`deny.toml`: `wrappers`).

| Port | Implementacja |
|---|---|
| `DesktopPort` (`desktop.rs`) | `EnumWindows` (kolejność Z), DWM (ramka, ukrycie), PID/obraz/`TokenElevation`, monitory z DPI, fokus, `SetWindowPos(SWP_ASYNCWINDOWPOS)`, `ShowWindowAsync`; strażnik tuż przed zmianą. |
| `UiaPort` (`uia/`) | wątek COM MTA z `recv_timeout` (porzucanie wiszących, limit), `IUIAutomation2` timeouts, `CacheRequest`, widok kontrolek, `RuntimeId`, wzorce, `TextPattern` tylko odczyt, pola haseł bez wartości. |
| `InputPort` (`input.rs`, `hook.rs`) | `SendInput` przez wspólny `execute_input`; hook LL wykrywa fizyczne wejście użytkownika (przerwanie). |
| `ScreenCapturePort` (`capture.rs`) | BitBlt/`PrintWindow`, maskowanie (okna chronione, deny-lista, pola haseł, okna niesprawdzone), PNG (`flate2`). |

`unsafe` tylko w modułach FFI z `#[allow(unsafe_code)]`; każdy blok ma `// SAFETY:`. Poza Windows porty zwracają
`Unsupported`. Testy Windows: `tests/gui_windows.rs` (pulpit `#[ignore]` — self-hosted).
