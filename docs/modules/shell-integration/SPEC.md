# shell-integration — SPEC (szkic v0)

## Cel
Integracja z powłoką Windows: ikona zasobnika (stany), toasty z AUMID przez stały launcher, autostart, handler protokołu `alfa://`, „Wyślij do" / „Otwórz w Alfie" z Eksploratora, pasek zadań (postęp długiego zadania, plakietka „czeka na zatwierdzenie"), tryb nie przeszkadzać (także automatycznie przy pełnym ekranie) (PLAN §3.3, §1.2, §14.8).

## Fala i priorytet
F1. P0. (Toasty renderuje `notify`; tu rejestracja i kanały systemowe.)

## Kontrakt (szkic Rust)
```rust
// shell-integration-contract — SZKIC
pub struct TrayIcon { pub state: TrayStatus, pub tooltip: String, pub menu: Vec<MenuItem> }
pub enum ProtocolAction { OpenSession(SessionId), NewChat { text: Option<String> }, ImportPackage(PathBuf), OpenFiles(Vec<PathBuf>) }
pub trait ShellIntegration: Send + Sync {
    fn set_tray(&self, icon: TrayIcon) -> Result<()>;
    fn register(&self, what: Registration /* Autostart | Protocol | SendTo | ContextMenu | Aumid */) -> Result<()>;
    fn unregister(&self, what: Registration) -> Result<()>;
    fn taskbar_progress(&self, p: Option<Progress>) -> Result<()>;
    fn taskbar_badge(&self, b: Option<Badge /* ApprovalPending | Count(u8) */>) -> Result<()>;
    fn on_protocol(&self) -> Subscription<ProtocolAction>;
    fn dnd(&self) -> DndState;
}
```
Zdarzenia: `shell.tray.clicked`, `shell.protocol.received`, `shell.sendto.received`, `shell.autostart.changed`, `shell.dnd.changed`, `shell.registration.failed`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (rejestr HKCU, zasobnik, ITaskbarList, AUMID), `device-profile-contract` (pełny ekran → DND), `updater-contract` (ścieżka stałego launchera `%LOCALAPPDATA%\Alfa\alfa.exe`).

## Niezmienniki
- Wszystkie rejestracje (skrót Menu Start, AUMID, protokół, „Wyślij do", autostart) wskazują **stały launcher**, nigdy katalog wersji (PLAN §1.2).
- Rejestracje tylko w HKCU (bez UAC; bez MSIX).
- Zatwierdzenia nigdy w toaście ani z zasobnika — tylko plakietka/przekierowanie do Broker-UI.
- Dane z protokołu i „Wyślij do" są niezaufanym wejściem: ścieżki walidowane, tekst oznacza sesję jako `tainted`, gdy pochodzi spoza Alfy (do ustalenia w SPEC v1 — PLAN nie rozstrzyga wprost).
- DND automatyczne przy pełnym ekranie/grach; ręczne DND ma pierwszeństwo.
- Odinstalowanie/rollback usuwa lub przepina rejestracje bez śladów.

## Zdolności / uprawnienia
Zapis rejestru HKCU (jądro, jako Ty); brak tokenów agentek.

## Izolacja
`inproc`, `always` (zasobnik żyje cały czas).

## Budżet zasobów
RAM ≤ 1 MB; zmiana ikony zasobnika ≤ 16 ms; brak pollingu.

## Konfiguracja (klucze TOML)
`[shell] autostart = false`, `start_minimized = true`, `protocol = true`, `send_to = true`, `context_menu_open_in_alfa = true`, `taskbar_progress = true`, `[shell.dnd] auto_fullscreen = true`.

## Wkład do UI
Ikona i menu zasobnika (z `ui-quick`), plakietki paska zadań, Ustawienia → Ogólne (autostart, integracje), makieta 20.

## Testy akceptacyjne
- `ACC-F1-shell-integration-01`: `alfa://` i „Wyślij do" trafiają do działającej instancji (single-instance) ≤ 500 ms (runner Windows).
- `ACC-F1-shell-integration-02`: toast z AUMID przez launcher działa po zmianie wersji side-by-side (spike j → test).
- `ACC-F1-shell-integration-03`: rejestracje wskazują stały launcher (test rejestru 100%).

## Fake
`shell-integration-fake`: rejestr i zasobnik w pamięci, skryptowane wywołania protokołu/„Wyślij do".

## Implementacja F1 (stan)
- **Zasobnik:** Tauri `tray-icon` — menu: Pokaż Alfę, Nowa rozmowa, Szybkie pytanie, Głos (przełącznik →
  `voice_set_mic_enabled`; bez modułu głosu cofa zaznaczenie), Nie przeszkadzać (wycisza toasty Windows),
  STOP WSZYSTKIEGO (`AppCore::system_kill_all(KillOrigin::Tray)`), Wyjście; lewy klik = okno główne.
  „Nowa rozmowa" tworzy sesję i otwiera ją w oknie głównym przez zdarzenie `OpenSession` (okno pokazuje
  rdzeń; przy błędzie zasobnik pokazuje samo okno). Stany ikony — F2.
- **Skróty globalne:** `Ctrl+Alt+Space` (Szybkie pytanie), `Ctrl+Shift+F12` (STOP:
  `system_kill_all(KillOrigin::Hotkey)` — anulowanie generacji i pobierań modeli, kill-switch Brokera
  (`BrokerPort::kill_all`: unieważnienie tokenów, drzewa procesów, wpis audytu `broker.kill_switch`),
  zatrzymanie mowy; ≤ 200 ms). Konflikt rejestracji → `Toast` w UI.
  Reguła AltGr i zarezerwowany kill-switch sprawdzane też w `settings_set_shortcut`.
- **Jedna instancja:** `tauri-plugin-single-instance` (pierwsza wtyczka) — druga instancja przekazuje
  argumenty; URI `alfa://` parsuje `app_core::protocol` z listą dozwolonych akcji: `open`, `quick`,
  `session/<id>` (id `[A-Za-z0-9_-]`), `new?text=` (tekst trafia do szkicu, nigdy nie jest wysyłany);
  reszta odrzucana (limit 2048 znaków, bez znaków sterujących). Rejestracja schematu: `plugins.deep-link`
  w `tauri.conf.json` (instalator). Single-instance przez mutex/IPC wtyczki — decyzja z pytania otwartego.
- **Przejście do sesji:** zdarzenie `OpenSession { sessionId }` (rdzeń → UI; `AppCore::open_session_in_ui`)
  pokazuje okno główne i przełącza je na sesję — używają go zasobnik („Nowa rozmowa"),
  `quick_expand_to_main` i URI `alfa://session/<id>`.
- **Dialogi plików:** `tauri-plugin-dialog` (=2.8.1) implementuje `ShellPort::pick_save_path`
  (filtr „Paczka Alfy" `*.alfa`), `pick_open_path` i „Zapisz jako" kodu; anulowanie dialogu = `None`
  (komenda kończy się bez błędu). Dialogi blokujące wołane z `spawn_blocking`, nigdy z wątku UI.
- **Logowanie do mostów CLI (F5):** `bridges_open_login` → `ShellPort::open_terminal` w katalogu domowym
  (bez wykonania polecenia); UI pokazuje polecenie do skopiowania (`claude /login`, `codex login`) także
  gdy terminala nie da się otworzyć. Alfa nie czyta ani nie przechowuje tokenów CLI.
- **Do F1+/launcher:** autostart, „Wyślij do", AUMID, pasek zadań; rejestracje mają wskazywać stały launcher
  (ADR 0007), którego jeszcze nie ma — dziś wskazują binarium instalatora.

## Otwarte pytania
- Single-instance: named pipe z ACL na SID vs mutex + WM_COPYDATA — do ustalenia w SPEC v1.
- Menu kontekstowe Eksploratora w Win11 (klasyczne vs IExplorerCommand wymagające pakietu) — spike j.
