//! Kontrakt `device-profile` (docs/modules/device-profile/SPEC.md, PLAN §3.5, §6.3):
//! profil sprzętu maszyny, klasa sprzętu, rekomendacja profilu głosu A–D i budżetu
//! `model-residency`, tryb baterii, emulacja baseline, nakładka per maszyna.
//!
//! Reguły (`classify`, `recommend`, `apply_overlay`, `Profile::emulate_baseline`) są czystymi
//! funkcjami tego crate'a — `-impl` i `-fake` dają identyczne rekomendacje dla tego samego profilu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod emulation;
pub mod fixtures;
mod overlay;
mod rules;
mod types;

use core_bus_contract::EventKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use overlay::{MachineOverlay, VoiceChoice, apply_overlay};
pub use rules::{
    DESKTOP_RESERVE_MB, HwClass, LocalLlm, Recommendation, ResidencyBudget, SttModel, VoiceProfile,
    VoiceVariant, classify, recommend, recommend_as,
};
pub use types::{
    AudioDevice, AudioDirection, BASELINE_LIMITS, Backend, BatteryInfo, CpuInfo, Emulation,
    EmulationFactors, GpuInfo, GpuVendor, MachineId, NpuInfo, OsInfo, PowerState, Profile,
    ResourceLimits,
};

/// Wykryto sprzęt (pierwsza detekcja w procesie).
pub const EVENT_DETECTED: &str = "device.detected";
/// Zmiana sprzętu (hot-plug, nowa karta).
pub const EVENT_CHANGED: &str = "device.changed";
/// Zmiana zasilania (sieć ↔ bateria).
pub const EVENT_POWER_CHANGED: &str = "device.power.changed";
/// Zmiana stanu aplikacji pełnoekranowej.
pub const EVENT_FULLSCREEN_CHANGED: &str = "device.fullscreen.changed";
/// Dławienie termiczne (F2).
pub const EVENT_THERMAL_THROTTLED: &str = "device.thermal.throttled";
/// Użytkownik nadpisał rekomendację (nakładka maszyny).
pub const EVENT_OVERRIDE: &str = "device.override";

/// Rodzaj zdarzenia jako `EventKind` magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenia modułu (ładunek zdarzenia na magistrali).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DeviceEvent {
    /// `device.detected`.
    Detected {
        /// Maszyna.
        machine_id: MachineId,
        /// Klasa.
        class: HwClass,
        /// Rekomendowany profil głosu.
        voice_profile: VoiceProfile,
    },
    /// `device.changed`.
    Changed {
        /// Maszyna.
        machine_id: MachineId,
        /// Nowa klasa.
        class: HwClass,
    },
    /// `device.power.changed`.
    PowerChanged {
        /// Nowy stan.
        power: PowerState,
    },
    /// `device.fullscreen.changed`.
    FullscreenChanged {
        /// Czy aplikacja pełnoekranowa jest aktywna.
        active: bool,
    },
    /// `device.override`.
    Override {
        /// Nowa nakładka.
        overlay: MachineOverlay,
    },
}

impl DeviceEvent {
    /// Nazwa zdarzenia na magistrali.
    pub fn name(&self) -> &'static str {
        match self {
            DeviceEvent::Detected { .. } => EVENT_DETECTED,
            DeviceEvent::Changed { .. } => EVENT_CHANGED,
            DeviceEvent::PowerChanged { .. } => EVENT_POWER_CHANGED,
            DeviceEvent::FullscreenChanged { .. } => EVENT_FULLSCREEN_CHANGED,
            DeviceEvent::Override { .. } => EVENT_OVERRIDE,
        }
    }
}

/// Błędy modułu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum DeviceProfileError {
    /// Identyfikator maszyny w złym formacie.
    #[error("niepoprawny identyfikator maszyny: {0}")]
    InvalidMachineId(String),
    /// Detekcja sprzętu nie powiodła się.
    #[error("detekcja sprzętu: {0}")]
    Detection(String),
    /// Zapis/odczyt stanu (plik identyfikatora maszyny).
    #[error("stan modułu: {0}")]
    Storage(String),
    /// Limity emulacji niepoprawne (zera).
    #[error("niepoprawne limity: {0}")]
    InvalidLimits(String),
}

/// Sprawdza limity emulacji (wszystkie > 0, wątki ≥ rdzenie).
pub fn validate_limits(limits: &ResourceLimits) -> Result<(), DeviceProfileError> {
    let l = limits;
    if l.cpu_cores == 0 || l.cpu_threads == 0 || l.ram_mb == 0 || l.vram_mb == 0 {
        return Err(DeviceProfileError::InvalidLimits(
            "wszystkie limity muszą być > 0".into(),
        ));
    }
    if l.cpu_threads < l.cpu_cores {
        return Err(DeviceProfileError::InvalidLimits(
            "wątków nie może być mniej niż rdzeni".into(),
        ));
    }
    Ok(())
}

/// Profil sprzętu bieżącej maszyny.
pub trait DeviceProfile: Send + Sync {
    /// Bieżący profil (z emulacją, jeśli włączona).
    fn current(&self) -> Profile;

    /// Rekomendacja dla bieżącego profilu z nakładką maszyny (`apply_overlay`).
    fn recommend(&self) -> Recommendation;

    /// Bieżący stan zasilania.
    fn power_state(&self) -> PowerState;

    /// Czy aktywna jest aplikacja pełnoekranowa (gra → STT/LLM na CPU lub chmurę).
    fn fullscreen_active(&self) -> bool;

    /// Włącza emulację limitów (`Some`) z korektą czasów gospodarza albo ją wyłącza (`None`).
    fn emulate(&self, limits: Option<ResourceLimits>) -> Result<(), DeviceProfileError>;

    /// Ponowna detekcja; `true`, jeśli sprzęt się zmienił (to samo `MachineId`).
    fn refresh(&self) -> Result<bool, DeviceProfileError>;
}

/// JSON Schema profilu (UI Kreatora sprzętu, eksport `transfer`).
pub fn profile_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(Profile)).unwrap_or_default()
}

/// JSON Schema rekomendacji.
pub fn recommendation_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(Recommendation)).unwrap_or_default()
}

/// JSON Schema zdarzeń modułu.
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(DeviceEvent)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_id_validation_and_serde() {
        let id = MachineId::parse("0123456789abcdef0123456789abcdef").unwrap();
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"0123456789abcdef0123456789abcdef\""
        );
        for bad in ["", "XYZ", "0123456789ABCDEF0123456789ABCDEF", "0123"] {
            assert!(MachineId::parse(bad).is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<MachineId>("\"nie-hex\"").is_err());
        assert_eq!(id.to_string(), id.as_str());
    }

    #[test]
    fn events_and_schemas() {
        let ev = DeviceEvent::PowerChanged {
            power: PowerState::Battery { percent: Some(42) },
        };
        assert_eq!(ev.name(), "device.power.changed");
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["event"], "power_changed");
        assert_eq!(json["power"]["source"], "battery");
        assert_eq!(event_kind(EVENT_DETECTED).as_str(), "device.detected");
        for schema in [profile_schema(), recommendation_schema(), event_schema()] {
            assert!(schema.get("$schema").is_some() || schema.get("title").is_some());
        }
        let override_ev = DeviceEvent::Override {
            overlay: MachineOverlay::default(),
        };
        assert_eq!(override_ev.name(), EVENT_OVERRIDE);
        let changed = DeviceEvent::Changed {
            machine_id: fixtures::desktop().machine_id,
            class: HwClass::StandardAmd,
        };
        assert_eq!(changed.name(), EVENT_CHANGED);
        assert_eq!(
            DeviceEvent::FullscreenChanged { active: true }.name(),
            EVENT_FULLSCREEN_CHANGED
        );
    }

    #[test]
    fn limits_validation() {
        assert!(validate_limits(&BASELINE_LIMITS).is_ok());
        let zero = ResourceLimits {
            ram_mb: 0,
            ..BASELINE_LIMITS
        };
        assert!(validate_limits(&zero).is_err());
        let inverted = ResourceLimits {
            cpu_cores: 8,
            cpu_threads: 4,
            ..BASELINE_LIMITS
        };
        assert!(validate_limits(&inverted).is_err());
    }
}
