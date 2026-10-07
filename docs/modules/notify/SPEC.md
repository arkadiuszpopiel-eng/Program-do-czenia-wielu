# notify — SPEC (szkic v0)

## Cel
Powiadomienia: natywne toasty Windows (gdy okno ukryte — zakończenie zadania, prośba o zatwierdzenie, błąd), toasty w aplikacji (prawy dolny róg, „Cofnij" 8 s), earcony (krótkie ciche dźwięki z osobną głośnością), tryb nie przeszkadzać, kolejkowanie i deduplikacja (PLAN §14.8).

## Fala i priorytet
F1. P0. Earcony przez `voice-audio` — od F2 (w F1 przez systemowe API dźwięku).

## Kontrakt (szkic Rust)
```rust
// notify-contract — SZKIC
pub enum Channel { WindowsToast, InApp, Earcon, TaskbarBadge }
pub enum Kind { TaskDone, ApprovalPending, Error, Undo { token: UndoToken, ttl: Duration }, Info, ListeningStart, ListeningStop }
pub struct Notification { pub id: NotifId, pub kind: Kind, pub title: String, pub body: String, pub session: Option<SessionId>,
    pub agent: Option<PersonaId>, pub actions: Vec<Action /* OpenSession | GoToBroker | Undo | Dismiss */>, pub priority: Priority, pub dedupe_key: Option<String> }
pub trait Notify: Send + Sync {
    fn send(&self, n: Notification) -> Result<NotifId>;
    fn dismiss(&self, id: NotifId) -> Result<()>;
    fn set_dnd(&self, on: bool, until: Option<Timestamp>) -> Result<()>;
    fn on_action(&self) -> Subscription<(NotifId, Action)>;
}
```
Zdarzenia: `notify.sent` (kanał), `notify.suppressed` (DND/dedupe), `notify.action` (kliknięcie), `notify.earcon.played`.

## Zależności
`core-bus/config/log-contract`, `shell-integration-contract` (AUMID, plakietki), `ui-shell-contract` (toasty w aplikacji, czy okno widoczne), `device-profile-contract` (pełny ekran), `voice-audio-contract` (F2, earcony przez mikser z duckingiem).

## Niezmienniki
- Toast nigdy nie zawiera przycisku zatwierdzającego — `ApprovalPending` ma tylko akcję „Przejdź do okna Brokera" (PLAN §14.8).
- Treść toastów nie zawiera sekretów ani treści niezaufanej bez skrócenia (redakcja jak w logach).
- Toast Windows tylko gdy okno główne ukryte/zminimalizowane; w aplikacji — toast wewnętrzny.
- DND wycisza wszystko poza `Error` krytycznym i `ApprovalPending` (plakietka zamiast toastu); earcony wyłączalne osobno.
- Dedupe po `dedupe_key` w oknie 5 s; kolejka nie rośnie bez ograniczeń (limit + zwijanie „N powiadomień").

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `always`.

## Budżet zasobów
RAM ≤ 1 MB; wysłanie ≤ 5 ms; earcon ≤ 50 ms do dźwięku.

## Konfiguracja (klucze TOML)
`[notify] windows_toasts = true`, `in_app = true`, `undo_toast_seconds = 8`, `[notify.earcons] enabled = true`, `volume = 0.3`, `[notify.dnd] manual = false`, `auto_fullscreen = true`, `[notify.kinds.<kind>] enabled = true`.

## Wkład do UI
Toasty w aplikacji (ui-kit `Toast`), karta „Cofnij", Ustawienia → Powiadomienia, przełącznik DND w zasobniku.

## Testy akceptacyjne
- `ACC-F1-notify-01`: test kontraktowy send/dismiss/dnd/dedupe na `-fake`/`-impl`.
- `ACC-F1-notify-02`: okno ukryte → toast Windows z AUMID; okno widoczne → toast w aplikacji (E2E runner).
- `ACC-F3-notify-03`: `ApprovalPending` nigdy nie ma akcji zatwierdzenia (test statyczny + E2E).

## Fake
`notify-fake`: kolejka w pamięci, skryptowane kliknięcia akcji, licznik earconów — do asercji w testach `agent-runtime`/`undo-journal`.

## Implementacja F1 (stan)
- `app_core::notify::native_notice` mapuje zdarzenia na toast Windows (tylko gdy okno główne jest
  ukryte): odpowiedź gotowa (`Stop end/max_tokens`), błąd (bez `offline`), prośba o zatwierdzenie
  („Otwórz okno Brokera" — **bez akcji zatwierdzania**, test), komunikat rdzenia ostrzeżenie/błąd.
  Treść bez fragmentów odpowiedzi; komunikat błędu skrócony do 160 znaków.
- Wysyłka: `tauri-plugin-notification` z pompy zdarzeń powłoki; tryb „nie przeszkadzać" z zasobnika
  wycisza toasty. Toasty w aplikacji = zdarzenie `Toast` (UI). Earcony, dedupe 5 s, AUMID przez launcher —
  w module `notify` (F1+/F2).
- Zdarzenia nawigacyjne i postępu (`OpenSession`, `LocalModelProgress`, `MicLevel`) nie dają toastu
  Windows. STOP WSZYSTKIEGO (`system_kill_all`) kończy się toastem w aplikacji (`Toast`, ostrzeżenie).
  Karta „Cofnij" kroku agentki niesie token `"<sesja>:u<krok>"` (`turns_undo_step` → `undo-journal`).
- F5–F7: raport dnia Marszałka (`MarshalReportReady`) → toast Windows „Alfa — raport dnia” (okno ukryte)
  i toast w aplikacji; eskalacje Marszałka → `Toast` (ostrzeżenie). Zadania (`TaskUpdated`), wyzwalacze
  (`TriggerFired`) i pamięć (`MemoryChanged`) — bez toastu Windows (stan w panelach).
- F8: `GuiActivity` (wskaźnik „agentka steruje” w pasku tytułu i panel Ekran), `SkillsChanged`
  i `HealthChanged` (strony Umiejętności i Zdrowie systemu) — bez toastu Windows (test w `app-api`);
  prośby o zgodę na sterowanie ekranem idą zwykłą ścieżką `ApprovalPending` → okno Brokera.

## Otwarte pytania
- Zestaw earconów (własne vs systemowe) i ich licencja — do ustalenia w SPEC v1.
