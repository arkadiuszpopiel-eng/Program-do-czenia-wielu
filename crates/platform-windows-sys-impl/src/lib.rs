//! Sygnały systemowe i obserwacja katalogów dla Windows (docs/modules/platform-windows/SPEC.md,
//! PLAN §3.4, §9.3, §9.5, §10) — implementacja portów `platform-contract`:
//!
//! - [`WinSignals`] (`SystemSignalsPort` + `IdlePort`, `PowerPort`, `FullscreenPort`, `SessionPort`):
//!   bezczynność `GetLastInputInfo` + `GetTickCount64`, zasilanie `GetSystemPowerStatus`, tryb gry
//!   `SHQueryUserNotificationState` + okno pierwszego planu pokrywające monitor (bez pulpitu, paska
//!   zadań i okien Alfy), sesja `WTSQuerySessionInformationW(WTSSessionInfoEx)`. Monitor: wątek z
//!   oknem komunikatów (`HWND_MESSAGE`), powiadomienia `WM_WTSSESSION_CHANGE` i
//!   `RegisterPowerSettingNotification` (zasilacz, poziom, oszczędzanie) budzą próbkę od razu,
//!   licznik `SetTimer` próbkuje bezczynność i pełny ekran (nie mają powiadomień). Histereza i filtr
//!   zmian: `SignalMonitor` z kontraktu (ten sam co w atrapie).
//! - [`WinDirWatch`] (`DirWatchPort`): `ReadDirectoryChangesW` z `OVERLAPPED` (wątek na obserwację,
//!   zatrzymanie zdarzeniem + `CancelIoEx`), bufor 64 KiB, przepełnienie (0 bajtów /
//!   `ERROR_NOTIFY_ENUM_DIR`) i przeniesienie podkatalogu → pełne przeskanowanie; deny-lista na
//!   ścieżce surowej i kanonicznej (junction do `.ssh` odrzucony), skan bez dowiązań i bez katalogów
//!   z deny-listy; debounce i filtr: `WatchSet` z kontraktu.
//!
//! Poza Windows: zapytania zwracają `Unsupported`, monitor i obserwacja się nie uruchamiają
//! (polityka obserwacji jest jednak sprawdzana wcześniej — testowalna na Linuksie).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
mod monitor_win;
mod notify;
#[cfg(not(windows))]
mod portable;
mod scan;
mod signals;
#[cfg(windows)]
mod sys_win;
mod watch;
#[cfg(windows)]
mod watch_win;
#[cfg(windows)]
mod win;

#[cfg(not(windows))]
use portable::{monitor, sys, watcher};
#[cfg(windows)]
use {monitor_win as monitor, sys_win as sys, watch_win as watcher};

pub use notify::{NotifyAction, parse_notify_buffer};
pub use scan::{Stat, scan_dir, stat_path};
pub use signals::WinSignals;
pub use watch::{DEFAULT_BUFFER_BYTES, DirWatchConfig, MIN_BUFFER_BYTES, WinDirWatch};
