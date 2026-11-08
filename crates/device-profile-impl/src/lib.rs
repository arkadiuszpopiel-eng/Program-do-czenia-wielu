//! Implementacja `device-profile` (docs/modules/device-profile/SPEC.md, PLAN §3.5, §6.3).
//!
//! Sprzęt czyta `HardwarePort` z `platform-contract`: na Windows podaje go kompozycja
//! (`platform-windows-impl` — DXGI, `GetSystemPowerStatus`, MMDevice, rejestr), poza Windows
//! `SysinfoProbe` (CPU/RAM z `sysinfo`, GPU brak). Moduł nie ma własnego kodu OS ani windows-rs.
//! Rekomendacje liczą czyste reguły kontraktu (`apply_overlay`), identyczne z `-fake`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod detect;
mod machine_id;
#[cfg(not(windows))]
mod sysinfo_probe;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};
use device_profile_contract::{
    DeviceEvent, DeviceProfile, DeviceProfileError, MachineOverlay, PowerState, Profile,
    Recommendation, ResourceLimits, apply_overlay, classify, event_kind, validate_limits,
};
use platform_contract::{HardwarePort, WindowPort};

pub use machine_id::{MACHINE_ID_FILE, derive_machine_id, resolve_machine_id};
#[cfg(not(windows))]
pub use sysinfo_probe::SysinfoProbe;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Konfiguracja usługi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProfileConfig {
    /// Katalog stanu (plik zapasowego `MachineId`), np. `%LOCALAPPDATA%\Alfa\state`.
    pub state_dir: PathBuf,
    /// Nakładka maszyny wczytana z `config/machine/<id>.toml`.
    pub overlay: MachineOverlay,
}

#[derive(Debug)]
struct State {
    detected: Profile,
    emulation: Option<ResourceLimits>,
    overlay: MachineOverlay,
    reported_power: PowerState,
    reported_fullscreen: bool,
    pending: Vec<DeviceEvent>,
}

/// Usługa profilu sprzętu (moduł `device-profile`).
pub struct DeviceProfileService {
    manifest: ModuleManifest,
    hw: Arc<dyn HardwarePort>,
    windows: Option<Arc<dyn WindowPort>>,
    state: Mutex<State>,
    bus: RwLock<Option<Arc<dyn EventBus>>>,
}

impl std::fmt::Debug for DeviceProfileService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceProfileService")
            .field("state", &*self.lock())
            .finish_non_exhaustive()
    }
}

impl DeviceProfileService {
    /// Wykrywa sprzęt (≤ 500 ms, raz) i ustala `MachineId`.
    pub fn detect(
        hw: Arc<dyn HardwarePort>,
        windows: Option<Arc<dyn WindowPort>>,
        config: DeviceProfileConfig,
    ) -> Result<Self, DeviceProfileError> {
        let manifest = ModuleManifest::parse_toml(MODULE_TOML)
            .map_err(|e| DeviceProfileError::Detection(format!("module.toml: {e}")))?;
        let seed = hw.machine_seed().ok().flatten();
        let id = resolve_machine_id(seed.as_deref(), &config.state_dir)?;
        let detected = detect::build_profile(hw.as_ref(), id);
        let reported_power = detected.power;
        Ok(Self {
            manifest,
            hw,
            windows,
            state: Mutex::new(State {
                detected,
                emulation: None,
                overlay: config.overlay,
                reported_power,
                reported_fullscreen: false,
                pending: Vec::new(),
            }),
            bus: RwLock::new(None),
        })
    }

    /// Detekcja sondą `sysinfo` (poza Windows; na Windows kompozycja podaje `HardwarePort`).
    #[cfg(not(windows))]
    pub fn detect_native(config: DeviceProfileConfig) -> Result<Self, DeviceProfileError> {
        Self::detect(Arc::new(SysinfoProbe), None, config)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn bus(&self) -> Option<Arc<dyn EventBus>> {
        self.bus.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Bieżąca nakładka maszyny.
    pub fn overlay(&self) -> MachineOverlay {
        self.lock().overlay.clone()
    }

    /// Ustawia nakładkę (nadpisanie użytkownika) i kolejkuje `device.override`.
    pub fn set_overlay(&self, overlay: MachineOverlay) {
        let mut st = self.lock();
        st.overlay = overlay.clone();
        st.pending.push(DeviceEvent::Override { overlay });
    }

    /// Sprawdza zasilanie i pełny ekran, kolejkuje zmiany i publikuje wszystkie oczekujące
    /// zdarzenia (`device.*`) na magistrali. Zwraca opublikowane (lub zebrane, gdy moduł stoi).
    pub async fn poll_changes(&self) -> Vec<DeviceEvent> {
        let power = self.power_state();
        let fullscreen = self.fullscreen_active();
        let events = {
            let mut st = self.lock();
            if power != st.reported_power {
                st.reported_power = power;
                st.pending.push(DeviceEvent::PowerChanged { power });
            }
            if fullscreen != st.reported_fullscreen {
                st.reported_fullscreen = fullscreen;
                st.pending
                    .push(DeviceEvent::FullscreenChanged { active: fullscreen });
            }
            std::mem::take(&mut st.pending)
        };
        if let Some(bus) = self.bus() {
            for event in &events {
                publish(bus.as_ref(), event).await;
            }
        }
        events
    }
}

async fn publish(bus: &dyn EventBus, event: &DeviceEvent) {
    let payload = serde_json::to_value(event).unwrap_or_default();
    // Błąd magistrali nie unieważnia detekcji (zdarzenia są informacyjne).
    let _ = bus
        .publish(Event::new(event_kind(event.name()), Level::Info, payload))
        .await;
}

impl DeviceProfile for DeviceProfileService {
    fn current(&self) -> Profile {
        let st = self.lock();
        match st.emulation {
            Some(limits) => st.detected.emulate(limits, st.detected.baseline_factors()),
            None => st.detected.clone(),
        }
    }

    fn recommend(&self) -> Recommendation {
        let overlay = self.overlay();
        apply_overlay(&self.current(), &overlay)
    }

    fn power_state(&self) -> PowerState {
        let (_, power) = detect::power_of(self.hw.power_status().ok());
        self.lock().detected.power = power;
        power
    }

    fn fullscreen_active(&self) -> bool {
        self.windows
            .as_ref()
            .is_some_and(|w| w.fullscreen_app_active())
    }

    fn emulate(&self, limits: Option<ResourceLimits>) -> Result<(), DeviceProfileError> {
        if let Some(l) = &limits {
            validate_limits(l)?;
        }
        self.lock().emulation = limits;
        Ok(())
    }

    fn refresh(&self) -> Result<bool, DeviceProfileError> {
        let id = self.lock().detected.machine_id.clone();
        let fresh = detect::build_profile(self.hw.as_ref(), id);
        let mut st = self.lock();
        let changed = detect::hardware_changed(&st.detected, &fresh);
        if changed {
            st.pending.push(DeviceEvent::Changed {
                machine_id: fresh.machine_id.clone(),
                class: classify(&fresh),
            });
        }
        st.detected = fresh;
        Ok(changed)
    }
}

#[async_trait]
impl Module for DeviceProfileService {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let slot = self.bus.get_mut().unwrap_or_else(|p| p.into_inner());
        if slot.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *slot = Some(Arc::clone(&ctx.bus));
        let profile = self.current();
        let recommendation = self.recommend();
        let detected = DeviceEvent::Detected {
            machine_id: profile.machine_id,
            class: recommendation.class,
            voice_profile: recommendation.voice_profile,
        };
        publish(ctx.bus.as_ref(), &detected).await;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus
            .get_mut()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .map(|_| ())
            .ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        if self.bus().is_none() {
            return HealthStatus::NotStarted;
        }
        let st = self.lock();
        if st.detected.ram_mb == 0 || st.detected.cpu.model.is_empty() {
            HealthStatus::Degraded("detekcja sprzętu niepełna (brak danych CPU/RAM)".into())
        } else {
            HealthStatus::Healthy
        }
    }
}
