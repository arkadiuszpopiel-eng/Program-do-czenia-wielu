//! Zasilanie (PLAN §3.5 tryb baterii, §10 „nie na baterii”): port stanu (Windows:
//! `GetSystemPowerStatus` + powiadomienia `RegisterPowerSettingNotification`), dekodowanie
//! surowej struktury `SYSTEM_POWER_STATUS` (testowalne bez Windows) i filtr zmian (bez zdarzenia
//! na każdy 1% baterii).

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;
use crate::hardware::PowerStatus;

/// Domyślny krok poziomu baterii zgłaszany jako zmiana (punkty procentowe).
pub const DEFAULT_PERCENT_STEP: u8 = 5;
/// Poziom „niski” — przekroczenie zawsze zgłaszane.
pub const LOW_BATTERY_PERCENT: u8 = 20;

/// Źródło zasilania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerSource {
    /// Sieć.
    Ac,
    /// Bateria.
    Battery,
    /// Nieznane.
    Unknown,
}

/// Stan zasilania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSnapshot {
    /// Źródło.
    pub source: PowerSource,
    /// Czy system ma baterię.
    pub battery_present: bool,
    /// Poziom 0–100, jeśli znany.
    pub battery_percent: Option<u8>,
    /// Oszczędzanie baterii / energii włączone.
    pub saver: bool,
    /// Szacowany czas pracy na baterii (s), jeśli znany.
    pub remaining_secs: Option<u32>,
}

impl PowerSnapshot {
    /// Stan nieznany (zasilanie nieodczytane).
    pub const UNKNOWN: Self = Self {
        source: PowerSource::Unknown,
        battery_present: false,
        battery_percent: None,
        saver: false,
        remaining_secs: None,
    };

    /// Zasilanie sieciowe bez baterii (komputer stacjonarny).
    pub const AC: Self = Self {
        source: PowerSource::Ac,
        ..Self::UNKNOWN
    };

    /// Na baterii z poziomem `percent`.
    pub fn battery(percent: u8) -> Self {
        Self {
            source: PowerSource::Battery,
            battery_present: true,
            battery_percent: Some(percent.min(100)),
            saver: false,
            remaining_secs: None,
        }
    }

    /// Dekoduje `SYSTEM_POWER_STATUS`: `ACLineStatus` (0 bateria, 1 sieć, 255 nieznane),
    /// `BatteryFlag` (128 brak baterii, 255 nieznane), `BatteryLifePercent` (255 nieznane),
    /// `SystemStatusFlag` (1 = oszczędzanie baterii), `BatteryLifeTime` (`u32::MAX` nieznane).
    pub fn from_system_power_status(
        ac_line: u8,
        battery_flag: u8,
        percent: u8,
        status_flag: u8,
        life_time: u32,
    ) -> Self {
        let source = match ac_line {
            0 => PowerSource::Battery,
            1 => PowerSource::Ac,
            _ => PowerSource::Unknown,
        };
        let battery_present = battery_flag != 128 && battery_flag != 255;
        Self {
            source,
            battery_present,
            battery_percent: (battery_present && percent <= 100).then_some(percent),
            saver: status_flag & 1 == 1,
            remaining_secs: (source == PowerSource::Battery && life_time != u32::MAX)
                .then_some(life_time),
        }
    }

    /// Czy na baterii.
    pub fn on_battery(&self) -> bool {
        self.source == PowerSource::Battery
    }

    /// Postać `HardwarePort` (dla `device-profile`).
    pub fn status(&self) -> PowerStatus {
        PowerStatus {
            ac_online: match self.source {
                PowerSource::Ac => Some(true),
                PowerSource::Battery => Some(false),
                PowerSource::Unknown => None,
            },
            battery_present: self.battery_present,
            battery_percent: self.battery_percent,
        }
    }

    /// Czy zmiana względem `previous` jest istotna: źródło, bateria, oszczędzanie, krok poziomu
    /// ≥ `step` albo przekroczenie progu niskiej baterii.
    pub fn differs_notably(&self, previous: &Self, step: u8) -> bool {
        if self.source != previous.source
            || self.battery_present != previous.battery_present
            || self.saver != previous.saver
        {
            return true;
        }
        match (self.battery_percent, previous.battery_percent) {
            (Some(a), Some(b)) => {
                a.abs_diff(b) >= step.max(1)
                    || (a <= LOW_BATTERY_PERCENT) != (b <= LOW_BATTERY_PERCENT)
            }
            (a, b) => a != b,
        }
    }
}

/// Port stanu zasilania.
pub trait PowerPort: Send + Sync {
    /// Bieżący stan.
    fn power(&self) -> Result<PowerSnapshot, PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_system_power_status() {
        let desktop = PowerSnapshot::from_system_power_status(1, 128, 255, 0, u32::MAX);
        assert_eq!(desktop, PowerSnapshot::AC);
        assert!(!desktop.on_battery());
        let laptop = PowerSnapshot::from_system_power_status(0, 0, 42, 1, 3_600);
        assert!(laptop.on_battery() && laptop.saver && laptop.battery_present);
        assert_eq!(laptop.battery_percent, Some(42));
        assert_eq!(laptop.remaining_secs, Some(3_600));
        assert_eq!(laptop.status().ac_online, Some(false));
        let charging = PowerSnapshot::from_system_power_status(1, 8, 80, 0, u32::MAX);
        assert_eq!(charging.remaining_secs, None);
        assert_eq!(charging.status().ac_online, Some(true));
        let unknown = PowerSnapshot::from_system_power_status(255, 255, 255, 0, u32::MAX);
        assert_eq!(unknown, PowerSnapshot::UNKNOWN);
        assert_eq!(unknown.status().ac_online, None);
    }

    #[test]
    fn notable_changes_only() {
        let a = PowerSnapshot::battery(60);
        assert!(!PowerSnapshot::battery(58).differs_notably(&a, 5));
        assert!(PowerSnapshot::battery(55).differs_notably(&a, 5));
        assert!(PowerSnapshot::battery(20).differs_notably(&PowerSnapshot::battery(22), 5));
        assert!(PowerSnapshot::AC.differs_notably(&a, 5));
        let saver = PowerSnapshot { saver: true, ..a };
        assert!(saver.differs_notably(&a, 5));
        let unknown_level = PowerSnapshot {
            battery_percent: None,
            ..a
        };
        assert!(unknown_level.differs_notably(&a, 5));
        assert_eq!(PowerSnapshot::battery(250).battery_percent, Some(100));
    }
}
