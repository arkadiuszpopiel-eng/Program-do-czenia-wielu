# platform-contract

Neutralny (bez `windows-rs`) kontrakt dostępu do systemu dla modułu `platform-windows`
(docs/PLAN.md §1.2, §3.2; docs/modules/platform-windows/SPEC.md). `SystemPort` to suma sub-traitów:
`FsPort`, `ProcessPort`, `ClipboardPort`, `WindowPort`, `HotkeyPort`, `TrayPort` — każdy moduł
korzysta z systemu wyłącznie przez nie. Każda operacja mutująca ma flagę `reversible`
(`FsOperation::is_reversible`, `OpReceipt`) i, jeśli odwracalna, `UndoToken` dla `FsPort::undo`.
Błędy: `PlatformError` (m.in. `Denylisted` dla ścieżek poświadczeń, `NotReversible`).
`Hotkey::validate` egzekwuje regułę AltGr (zakaz `Ctrl+Alt(+Shift)` + a, c, e, l, n, o, s, x, z)
i rezerwuje kill-switch `Ctrl+Shift+F12`. Implementacje: `platform-fake` (Linux/CI), `platform-windows-impl` (F1).
