# platform-windows — SPEC (szkic v0)

## Cel
Jedyny crate z windows-rs/COM, zamknięty za traitem `SystemPort`: pliki, procesy, schowek, okna, zasobnik, skróty globalne, hook klawiatury (PTT), ścieżki i ACL. Wszystkie moduły korzystają z systemu **wyłącznie** przez ten kontrakt (PLAN §1.2, §3.2). UIA i SendInput dochodzą w v1.5/v2.

## Fala i priorytet
F0: `SystemPort`-contract + fake (pkt 2 w §4.5a). F1: v1 (fs, procesy, schowek, okna, zasobnik, skróty globalne + `WH_KEYBOARD_LL`; bez UIA/SendInput). F5: v1.5 (SendInput tekstu, UIA `TextPattern` odczyt). F6: v2 (UIA, SendInput, zrzuty). P0.

## Kontrakt (szkic Rust)
```rust
// platform-windows-contract — SZKIC (trait SystemPort dzielony na pod-porty)
pub trait SystemPort: FsPort + ProcessPort + ClipboardPort + WindowPort + ShellPort + HotkeyPort {}
pub trait FsPort { fn read(&self, p: &Path) -> Result<Vec<u8>>; fn write_atomic(&self, p: &Path, data: &[u8]) -> Result<()>;
    fn move_to_recycle_bin(&self, p: &Path) -> Result<()>; fn watch(&self, p: &Path) -> Subscription<FsChange>;
    fn known_folder(&self, k: KnownFolder) -> PathBuf; }
pub trait ProcessPort { fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle>;   // Job Object, token, stdio/pipe
    fn kill_tree(&self, h: &ProcessHandle) -> Result<()>; fn foreground_is_elevated(&self) -> bool; }
pub trait ClipboardPort { fn get(&self) -> Result<ClipboardContent>; fn set(&self, c: ClipboardContent) -> Result<()>;
    fn watch(&self) -> Subscription<ClipboardChange>; }
pub trait WindowPort { fn list(&self) -> Vec<WindowInfo>; fn focus(&self, id: WindowId) -> Result<()>;
    fn fullscreen_app_active(&self) -> bool; }
pub trait HotkeyPort { fn register(&self, combo: Hotkey) -> Result<Subscription<HotkeyEvent>>;   // z regułą AltGr
    fn ptt_hook(&self) -> Result<Subscription<KeyState>>; }                                    // WH_KEYBOARD_LL
pub struct ProcessSpec { pub cmd: PathBuf, pub args: Vec<OsString>, pub cwd: PathBuf,
                         pub integrity: Integrity, pub job: JobLimits, pub app_container: bool }
```
Zdarzenia: `platform.hotkey`, `platform.ptt`, `platform.clipboard.changed`, `platform.fs.changed`, `platform.device.changed`, `platform.fullscreen.changed`, `platform.session.locked`.

## Zależności
`core-bus-contract`, `core-log-contract`. Zewnętrzne: windows-rs (jedna wersja, `docs/vendor/windows-rs.md`).

## Niezmienniki
- Poza tym crate'em brak `windows`/`windows-sys` w workspace (test CI grafu zależności).
- Skrót globalny naruszający regułę AltGr (`Ctrl+Alt(+Shift)` + a, c, e, l, n, o, s, x, z) jest odrzucany (PLAN §8.6; test CI).
- Usuwanie plików domyślnie do Kosza; `write_atomic` = tmp + rename.
- COM/UIA na dedykowanym wątku MTA; brak wywołań COM z wątku audio RT.
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
- Podział `SystemPort` na osobne crate'y kontraktowe per pod-port (ładowanie leniwe) — do ustalenia w SPEC v1.
- Snap Layouts/Mica przez Tauri (spike j) — czy własny pasek tytułu wymaga kodu tutaj.
