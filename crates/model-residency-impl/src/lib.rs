//! Zarządca rezydencji modeli w RAM/VRAM (docs/modules/model-residency/SPEC.md, PLAN §3.4, §3.5).
//!
//! - [`ResidencyManager`] — implementacja `Residency` na wspólnej maszynie stanów kontraktu
//!   (`LeaseTable`), zegar monotoniczny, słuchacze właścicieli (eksmisja/przeniesienie na CPU);
//! - [`ResidencyModule`] — moduł rejestru: zdarzenia `residency.*` na magistralę (w kolejności),
//!   zadanie tła co `tick`: zwalnianie bezczynnych i odświeżanie trybu z sygnałów
//!   (pełny ekran → gra, bateria) — [`DeviceSignals`] nad `device-profile`;
//! - [`ResidencyConfig`] — `[machine.residency]` (`"auto"` = z rekomendacji `device-profile`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod config;
mod manager;
mod module;

pub use config::{Auto, Masked, ResidencyConfig, Switch, parse_duration};
pub use manager::{DeviceSignals, MonotonicClock, ResidencyManager};
pub use module::{MODULE_TOML, ResidencyModule};
