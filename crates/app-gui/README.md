# app-gui

Computer use w aplikacji (kategoria `app-*`).

- `GuiPorts::system(local)` — porty `WinGui` (pulpit, UIA, wejście, zrzuty) tworzone leniwie ze
  strażnikiem `TargetGuard::baseline().with_pids([Alfa + potomkowie: WebView2, Broker-UI, watchdog])
  .with_image_dirs([%LOCALAPPDATA%\Alfa, katalog programu])`; `GuiPorts::from_one` — atrapa
  `platform-fake` w testach.
- `gui_tools` — narzędzia `tools-window` / `tools-uia` / `tools-input` / `tools-screen` (grupa
  `gui.control`, tylko role z tą grupą) owinięte `WatchedTool`: każda akcja przez Brokera, wobec
  okien Alfy — blokada Jądra niezależnie od zgód.
- `GuiMonitor` — panel „Ekran": kto steruje, ostatnie 20 akcji **bez wpisywanej treści** („Wpisz tekst
  (N znaków)"), ostatni zamaskowany zrzut tylko w pamięci (piksele wyłącznie przez `gui_screenshot`,
  nigdy w zdarzeniach); `take_over` („Zatrzymaj sterowanie") anuluje akcje w toku i wstrzymuje
  narzędzia GUI do `release`; zdarzenie `GuiActivity` (metadane).
- `GuiApp::desktop_grant` — „zawsze zezwalaj na podgląd pulpitu": `gui.control(desktop.exe)` w zakresie
  ≤ 24 h — decyzja wyłącznie w oknie Brokera; niewykorzystany token odwoływany po czasie.

Okna Alfy dodatkowo chroni powłoka: `WDA_EXCLUDEFROMCAPTURE` (`content_protected(true)`).
Testy: `tests/screen.rs` (Notatnik vs okna Alfy, 200 losowych prób, zrzut poza zdarzeniami,
przejęcie), `crates/app-core/tests/computer.rs` (przez komendy, bez zgody — odmowa).
