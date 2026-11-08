//! Atrapa `DeviceProfile` (docs/modules/device-profile/SPEC.md, „Fake”): profil z fixture'ów,
//! zdarzenia sterowane przez test, rekomendacje z reguł kontraktu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Mutex, MutexGuard};

use device_profile_contract::{
    DeviceEvent, DeviceProfile, DeviceProfileError, MachineOverlay, PowerState, Profile,
    Recommendation, ResourceLimits, apply_overlay, classify, fixtures, validate_limits,
};

#[derive(Debug)]
struct State {
    detected: Profile,
    emulation: Option<ResourceLimits>,
    overlay: MachineOverlay,
    fullscreen: bool,
    events: Vec<DeviceEvent>,
    pending_hot_plug: Option<Profile>,
}

/// Atrapa profilu sprzętu.
#[derive(Debug)]
pub struct FakeDeviceProfile {
    state: Mutex<State>,
}

impl FakeDeviceProfile {
    /// Atrapa startująca z danego profilu.
    pub fn new(profile: Profile) -> Self {
        Self {
            state: Mutex::new(State {
                detected: profile,
                emulation: None,
                overlay: MachineOverlay::default(),
                fullscreen: false,
                events: Vec::new(),
                pending_hot_plug: None,
            }),
        }
    }

    /// Maszyna baseline (RX 7600 8 GB, 16 GB, 6c/12t).
    pub fn baseline() -> Self {
        Self::new(fixtures::baseline())
    }

    /// Desktop Standard-AMD (RX 9070 XT 16 GB, 32 GB, 8c/16t).
    pub fn desktop() -> Self {
        Self::new(fixtures::desktop())
    }

    /// Laptop Laptop-CUDA (RTX 4050 6 GB, 16 GB, 14c/20t, bateria).
    pub fn laptop() -> Self {
        Self::new(fixtures::laptop())
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zmienia zasilanie (zdarzenie `device.power.changed`, jeśli stan się zmienił).
    pub fn set_power(&self, power: PowerState) {
        let mut st = self.lock();
        if st.detected.power != power {
            st.detected.power = power;
            st.events.push(DeviceEvent::PowerChanged { power });
        }
    }

    /// Zmienia stan pełnego ekranu (zdarzenie `device.fullscreen.changed`).
    pub fn set_fullscreen(&self, active: bool) {
        let mut st = self.lock();
        if st.fullscreen != active {
            st.fullscreen = active;
            st.events.push(DeviceEvent::FullscreenChanged { active });
        }
    }

    /// Podmienia sprzęt; zmiana jest widoczna po `refresh()` (jak prawdziwa detekcja).
    /// `MachineId` zostaje stary — to ta sama maszyna.
    pub fn hot_plug(&self, profile: Profile) {
        self.lock().pending_hot_plug = Some(profile);
    }

    /// Ustawia nakładkę użytkownika (zdarzenie `device.override`).
    pub fn set_overlay(&self, overlay: MachineOverlay) {
        let mut st = self.lock();
        st.overlay = overlay.clone();
        st.events.push(DeviceEvent::Override { overlay });
    }

    /// Zabiera zapisane zdarzenia (FIFO).
    pub fn drain_events(&self) -> Vec<DeviceEvent> {
        std::mem::take(&mut self.lock().events)
    }
}

impl DeviceProfile for FakeDeviceProfile {
    fn current(&self) -> Profile {
        let st = self.lock();
        match st.emulation {
            Some(limits) => st.detected.emulate(limits, st.detected.baseline_factors()),
            None => st.detected.clone(),
        }
    }

    fn recommend(&self) -> Recommendation {
        let overlay = self.lock().overlay.clone();
        apply_overlay(&self.current(), &overlay)
    }

    fn power_state(&self) -> PowerState {
        self.lock().detected.power
    }

    fn fullscreen_active(&self) -> bool {
        self.lock().fullscreen
    }

    fn emulate(&self, limits: Option<ResourceLimits>) -> Result<(), DeviceProfileError> {
        if let Some(l) = &limits {
            validate_limits(l)?;
        }
        self.lock().emulation = limits;
        Ok(())
    }

    fn refresh(&self) -> Result<bool, DeviceProfileError> {
        let mut st = self.lock();
        let Some(mut next) = st.pending_hot_plug.take() else {
            return Ok(false);
        };
        next.machine_id = st.detected.machine_id.clone();
        next.power = st.detected.power;
        let changed = next != st.detected;
        if changed {
            st.events.push(DeviceEvent::Changed {
                machine_id: next.machine_id.clone(),
                class: classify(&next),
            });
            st.detected = next;
        }
        Ok(changed)
    }
}
