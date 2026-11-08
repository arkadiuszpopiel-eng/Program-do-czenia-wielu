//! Stan maszyny z `device-profile` (zasilanie, pełny ekran), licznika bezczynności (platforma)
//! i zegara lokalnego.

use std::sync::Arc;

use chrono::NaiveTime;
use device_profile_contract::{DeviceProfile, PowerState};
use memory_consolidation_contract::{HostConditions, HostState};

/// Licznik bezczynności użytkownika (Windows: `GetLastInputInfo` w `platform-windows`).
pub trait IdleSource: Send + Sync {
    /// Sekundy od ostatniej aktywności użytkownika.
    fn idle_secs(&self) -> u64;
}

/// Bezczynność nieznana → traktowana jako aktywność (bezpieczniej: harmonogram nie startuje).
#[derive(Debug, Clone, Copy, Default)]
pub struct UnknownIdle;

impl IdleSource for UnknownIdle {
    fn idle_secs(&self) -> u64 {
        0
    }
}

/// Zegar lokalny (okno nocne liczone w czasie lokalnym).
pub trait LocalClock: Send + Sync {
    /// Teraz (czas lokalny).
    fn local_time(&self) -> NaiveTime;
}

/// Zegar lokalny systemu.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemLocalClock;

impl LocalClock for SystemLocalClock {
    fn local_time(&self) -> NaiveTime {
        chrono::Local::now().time()
    }
}

/// Stan maszyny z `DeviceProfile`. Zasilanie nieznane traktowane jak sieć (stacjonarny).
pub struct DeviceHost {
    device: Arc<dyn DeviceProfile>,
    idle: Arc<dyn IdleSource>,
    clock: Arc<dyn LocalClock>,
}

impl DeviceHost {
    /// Nowy adapter.
    pub fn new(
        device: Arc<dyn DeviceProfile>,
        idle: Arc<dyn IdleSource>,
        clock: Arc<dyn LocalClock>,
    ) -> Self {
        Self {
            device,
            idle,
            clock,
        }
    }
}

impl HostConditions for DeviceHost {
    fn state(&self) -> HostState {
        HostState {
            on_battery: matches!(self.device.power_state(), PowerState::Battery { .. }),
            fullscreen: self.device.fullscreen_active(),
            idle_secs: self.idle.idle_secs(),
            local_time: self.clock.local_time(),
        }
    }
}
