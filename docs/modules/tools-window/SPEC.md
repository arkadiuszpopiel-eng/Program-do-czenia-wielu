# tools-window — SPEC (v1: narzędzia zaimplementowane, F6)

## Cel
Narzędzia agentek do okien: lista okien i monitorów, fokus, położenie/rozmiar, minimalizacja/maksymalizacja/przywracanie (PLAN §7.2 „okna i pulpity”). Moduł `gui` w `tools-window-contract` to **wspólna bramka GUI** dla `tools-uia`, `tools-input`, `tools-screen`. Nie steruje oknami Alfy/Brokera/helpera (THREAT_MODEL S11, §7).

## Fala i priorytet
F6 (computer use), P1. v1: cztery narzędzia + bramka. Pulpity wirtualne — później (SPEC v2).

## Kontrakt
```rust
window_list   { include_minimized? }                         → ListOutput { windows: [WindowBrief], foreground?, monitors, hidden_protected }
window_focus  { window }                                     → ChangeOutput { before, after, verified }
window_move   { window, x, y, width, height }                → ChangeOutput   // ≥ 64×64, musi zostać na monitorze
window_state  { window, state: minimized|maximized|normal }  → ChangeOutput
// gui (wspólne): DESKTOP_APP = "desktop.exe", target_window, app_capability, desktop_capability,
//   authorize (decide → zatwierdzenie → verify), gui_outcome(GuiError), brief, emit_verify ("tool.gui.verify"), blocking
pub struct WindowTools; impl WindowTools { pub fn new(WindowToolsDeps { desktop: DesktopPort, broker, bus }) }
```
Zdarzenia: `tool.window.list` (liczba okien), `tool.window.change` (okno, aplikacja, rodzaj zmiany), `tool.gui.verify` (krok weryfikacji, F6-04) — bez tytułów.

## Zależności
`tools-common-contract` (manifest, `BrokerGate`), `safety-broker-contract` (`Capability::GuiControl(AppSelector)`), `platform-contract` (`DesktopPort`, `TargetGuard`, `GuiError`), `core-bus-contract`.

## Niezmienniki
- Okno chronione (`DesktopWindow::protected` albo `TargetGuard::is_protected`) → odmowa `KernelBlock(GuiControlOfKernelProcess)` **przed** Brokerem; port sprawdza strażnikiem jeszcze raz tuż przed wywołaniem systemu.
- Lista: okna chronione pominięte (`hidden_protected`), tytuły redagowane (`redact_secrets`) i niezaufane (taint `Screen` zgłaszany Brokerowi).
- Każda zmiana: Broker `gui.control(<plik wykonywalny okna>)`; lista: `gui.control(desktop.exe)` z faktem „dane prywatne”.
- Po każdej zmianie krok weryfikacji (stan okna = zamiar, do 1 s na zmiany asynchroniczne) jako zdarzenie.
- Argumenty tylko jako obiekt JSON zgodny ze schematem (`additionalProperties: false`).

## Zdolności / uprawnienia
`gui.control(<aplikacja>)`, `gui.control(desktop.exe)` (pseudo-aplikacja pulpitu). Odwracalność: lista `yes`, zmiany `scoped` (wynik niesie położenie sprzed zmiany).

## Izolacja
`inproc`, `lazy`; wywołania portu na `spawn_blocking`.

## Budżet zasobów
RAM ≤ 4 MB; lista ≤ 50 ms; weryfikacja ≤ 1 s.

## Konfiguracja (klucze TOML)
Brak własnych (strażnik celów: `[platform.gui]` w `platform-windows`).

## Wkład do UI
Kroki w wątku/Replay (zdarzenia), panel „Ekran” (F6, inna sesja).

## Testy akceptacyjne
- `ACC-F6-tools-window-01`: 0 zmian okien Alfy/Brokera/helpera w 200 losowych próbach (property, `tests/window.rs`).
- `ACC-F6-tools-window-02`: weryfikacja po każdej zmianie (F6-04) — zdarzenie `tool.gui.verify` 3/3.
- F6-06: odmowa wobec okien chronionych (fokus, położenie, stan) bez kontaktu z Brokerem.

## Fake
`tools-window-fake`: prawdziwe manifesty i walidacja argumentów, wyniki skryptowane, zapis wywołań.

## Otwarte pytania
- Pulpity wirtualne (`IVirtualDesktopManager`) i przenoszenie okien między monitorami — SPEC v2.
- Lista aplikacji „zawsze zezwalaj” dla `gui.control` (L3) — Ustawienia → Uprawnienia (Broker).
