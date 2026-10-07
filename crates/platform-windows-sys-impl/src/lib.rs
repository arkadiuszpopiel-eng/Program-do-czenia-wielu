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
//! - [`WinSys`] (`SysPort` z `platform-apps-contract`, F6 `tools-system`): procesy (Toolhelp32,
//!   zakończenie przez uchwyt po sprawdzeniu tożsamości, strażnika celów i właściciela), usługi
//!   (SCM), Dziennik zdarzeń (`EvtQuery`), zmienne (`HKCU\Environment`, `WM_SETTINGCHANGE`);
//! - [`DiskDownloads`] (`DownloadStore`, F6 `tools-net`): kwarantanna pobrań bez dowiązań, nowy plik,
//!   MOTW, nazwa końcowa bez nadpisania.
//!
//! Poza Windows: zapytania zwracają `Unsupported`, monitor i obserwacja się nie uruchamiają
//! (polityka obserwacji jest jednak sprawdzana wcześniej — testowalna na Linuksie).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![cfg_attr(not(windows), allow(dead_code))]

mod downloads;
#[cfg(windows)]
mod env_win;
#[cfg(windows)]
mod events_win;
mod events_xml;
#[cfg(windows)]
mod monitor_win;
mod notify;
#[cfg(not(windows))]
mod portable;
#[cfg(windows)]
mod procs_win;
mod scan;
#[cfg(windows)]
mod services_win;
mod signals;
#[cfg(windows)]
mod sys_win;
mod sysport;
mod watch;
#[cfg(windows)]
mod watch_win;
#[cfg(windows)]
mod win;

#[cfg(not(windows))]
use portable::{monitor, sys, watcher};
#[cfg(windows)]
use {monitor_win as monitor, sys_win as sys, watch_win as watcher};

pub use downloads::DiskDownloads;
pub use notify::{NotifyAction, parse_notify_buffer};
pub use scan::{Stat, scan_dir, stat_path};
pub use signals::WinSignals;
pub use sysport::WinSys;
pub use watch::{DEFAULT_BUFFER_BYTES, DirWatchConfig, MIN_BUFFER_BYTES, WinDirWatch};
