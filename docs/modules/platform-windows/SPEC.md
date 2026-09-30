# platform-windows — SPEC (szkic v0)

## Cel
Jedyny crate z windows-rs/COM, zamknięty za traitem `SystemPort`: pliki, procesy, schowek, okna, zasobnik, skróty globalne, hook klawiatury (PTT), ścieżki i ACL. Wszystkie moduły korzystają z systemu **wyłącznie** przez ten kontrakt (PLAN §1.2, §3.2). UIA i SendInput dochodzą w v1.5/v2.

## Fala i priorytet
F0: `SystemPort`-contract + fake (pkt 2 w §4.5a). F1 (zrobione: `platform-windows-impl`): v1 (fs, procesy, schowek, okna, zasobnik, skróty globalne + `WH_KEYBOARD_LL`; bez UIA/SendInput). F5: v1.5 (SendInput tekstu, UIA `TextPattern` odczyt). F6: v2 (UIA, SendInput, zrzuty). P0.

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

## Zależności
`platform-contract` (F1 bez magistrali — zdarzenia publikuje jądro). Zewnętrzne: `windows`/`windows-core` 0.62.2 (jedna wersja, `docs/vendor/windows.md`).

## Niezmienniki
- Poza tym crate'em brak `windows`/`windows-sys` w workspace (test CI grafu zależności).
- Skrót globalny naruszający regułę AltGr (`Ctrl+Alt(+Shift)` + a, c, e, l, n, o, s, x, z) jest odrzucany (PLAN §8.6; test CI).
- Usuwanie plików domyślnie do Kosza; `write_atomic` = tmp + rename.
- COM/UIA na dedykowanym wątku MTA; brak wywołań COM z wątku audio RT. F1: `IFileOperation` na krótkotrwałym wątku STA, MMDevice na wątku MTA, skróty i hook `WH_KEYBOARD_LL` na własnym wątku z pętlą komunikatów.
- Deny-lista sprawdzana na postaci surowej, po `%ZMIENNYCH%`, leksykalnej (`\\?\`, wielkość liter, końcowe kropki/spacje, ADS) i kanonicznej (dowiązania, junctions, 8.3); kopiowanie drzewa sprawdza każdy wpis.
- Usuwanie do Kosza, którego nie da się przenieść do Kosza, pyta użytkownika (`FOF_WANTNUKEWARNING`); zgoda = pokwitowanie nieodwracalne.
- Deny-lista ścieżek poświadczeń (`~/.claude`, `~/.codex`, profile przeglądarek, Credential Manager) egzekwowana tu jako ostatnia linia (nawet gdy `tools-fs` zawiedzie).
- Hooki/PTT nie działają przy oknie administratora na pierwszym planie bez helpera `uiAccess` → `foreground_is_elevated()` i zdarzenie do UI (PLAN §7.3).

## Zdolności / uprawnienia
Port sam nie decyduje: wykonuje operacje z tokenem zdolności przekazanym przez wywołującego (od F3); w F1 (bez Brokera) tylko wywołania jądra i UI.

## Izolacja
`inproc`, `always` (część `process` — helper `uiAccess` w F6, osobny SPEC).

## Budżet zasobów
RAM ≤ 4 MB; `spawn` ≤ 30 ms; reakcja hooka PTT ≤ 10 ms; watchery bez pollingu.

## Konfiguracja (klucze TOML)
`[platform] recycle_bin = true`, `[platform.hotkeys] kill_switch = "Ctrl+Shift+F12"`, `quick_ask = "Ctrl+Alt+Space"`, `ptt = "Space"`; `[platform.denylist_paths] = [...]` (kernel_policy).

## Wkład do UI
Brak własnego; dostarcza zasobnik/okna dla `shell-integration` i `ui-quick`, stan „okno admina na wierzchu" dla paska stanu.

## Testy akceptacyjne
- `ACC-F0-platform-windows-01`: kontrakt `SystemPort` przechodzi na `-fake` (wirtualny FS) i `-impl` (self-hosted runner).
- `ACC-F1-platform-windows-02`: reguła AltGr — zestaw zakazanych skrótów odrzucony 100%.
- `ACC-F1-platform-windows-03`: PTT — puszczenie klawisza zgłoszone ≤ 10 ms (hook), na runnerze desktop.
- `ACC-F1-platform-windows-04`: `kill_tree` zabija całe drzewo (Job Object) ≤ 50 ms.

## Fake
`platform-windows-fake`: wirtualny system plików, schowek i lista okien w pamięci, skrypty zdarzeń (hotkey/PTT/hot-plug) z wirtualnym zegarem — testy na Linux/CI bez Windows.

## Otwarte pytania
- Kontrakt F0 nie ma `watch` (FS, schowek) ani subskrypcji zdarzeń skrótów — do dodania w SPEC v1 (zdarzenia na magistrali).
- Rozszerzyć kontrakt: `WindowInfo` o PID/prostokąt/DPI, `WindowPort` o minimalizację, `ProcessSpec` o limity CPU/affinity i stdio (dziś metody impl).
- Dziennik cofnięć FS jest w pamięci procesu — trwały dziennik to `undo-journal` (F3).
- Podział `SystemPort` na osobne crate'y kontraktowe per pod-port (ładowanie leniwe) — do ustalenia w SPEC v1.
- Snap Layouts/Mica przez Tauri (spike j) — czy własny pasek tytułu wymaga kodu tutaj.
