//! Porty aplikacji F6 dla Windows (docs/modules/platform-apps/SPEC.md, PLAN §7.1–7.2, §8.5) —
//! implementacja `platform-apps-contract`:
//!
//! - [`WinOffice`] (`OfficePort`): Word/Excel przez COM (`IDispatch`, wątek STA z limitem czasu,
//!   porzucanie wiszących wątków), kopia robocza w prywatnym katalogu, `AutomationSecurity = 3`
//!   z odczytem kontrolnym, Protected View dla plików z Internetu, Excel w trybie obliczeń ręcznym;
//! - [`CdpBrowser`] (`BrowserPort`): Edge/Chrome z osobnym profilem Alfy, CDP przez potok
//!   (deskryptory CRT 3/4), każde żądanie przez filtr egressu (`Fetch.requestPaused`), WebSockety
//!   zablokowane, cele potomne wstrzymane do włączenia przechwytywania, pobrania w kwarantannie,
//!   Job Object;
//! - [`WinRegistry`] (`RegistryPort`): `HKCU`/`HKLM` tylko do odczytu, deny-lista przed otwarciem.
//!
//! `unsafe` wyłącznie w modułach FFI (`#[allow(unsafe_code)]`, każdy blok z `// SAFETY:`). Poza
//! Windows Office i rejestr zwracają błąd „nieobsługiwane”, a przeglądarka — „niedostępna”
//! (logika CDP jest przenośna i testowana na atrapie protokołu).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod browser;
mod office;
mod registry;

#[cfg(windows)]
pub use browser::WinLauncher;
pub use browser::{BrowserConfig, CdpBrowser, Launched, Launcher, NoLauncher, ProcessGuard};
pub use office::{OfficeConfig, WinOffice, WorkCopy, ZONE_INTERNET, zone_of_path, zone_stream};
pub use registry::{WinRegistry, decode_value};
