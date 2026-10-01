//! Konfiguracja maszyny `[machine.residency]` (SPEC): `vram_mb = "auto"`, `ram_mb = "auto"`,
//! `desktop_reserve_mb = 768`, `gaming_mode = "auto"`, `battery_mode = "auto"`, `tick = "30s"`.

use std::time::Duration;

use device_profile_contract::ResidencyBudget;
use model_residency_contract::{Budget, ModeSource};
use serde::Deserialize;

/// Wartość liczbowa albo `"auto"` (z rekomendacji `device-profile`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(try_from = "toml::Value")]
pub enum Auto {
    /// Z rekomendacji.
    #[default]
    Auto,
    /// Jawna wartość (MB).
    Mb(u32),
}

impl TryFrom<toml::Value> for Auto {
    type Error = String;

    fn try_from(v: toml::Value) -> Result<Self, Self::Error> {
        match v {
            toml::Value::String(s) if s == "auto" => Ok(Auto::Auto),
            toml::Value::Integer(i) => u32::try_from(i)
                .map(Auto::Mb)
                .map_err(|_| format!("wartość {i} poza zakresem")),
            other => Err(format!("oczekiwano liczby MB albo \"auto\", jest {other}")),
        }
    }
}

/// Przełącznik trybu: automatyczny (z sygnałów) albo wyłączony.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Switch {
    /// Z sygnałów `device-profile`.
    #[default]
    Auto,
    /// Nigdy.
    Off,
}

/// Konfiguracja zarządcy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResidencyConfig {
    /// VRAM dla modeli (po rezerwie) albo `auto`.
    pub vram_mb: Auto,
    /// RAM dla modeli albo `auto`.
    pub ram_mb: Auto,
    /// Rezerwa VRAM pulpitu (MB); brak = z rekomendacji (768).
    pub desktop_reserve_mb: Option<u32>,
    /// Tryb gry z sygnału pełnego ekranu.
    pub gaming_mode: Switch,
    /// Tryb baterii z sygnału zasilania.
    pub battery_mode: Switch,
    /// Okres zadania tła (bezczynność, odświeżenie trybu), np. `"30s"`.
    #[serde(with = "duration_text")]
    pub tick: Duration,
}

impl Default for ResidencyConfig {
    fn default() -> Self {
        Self {
            vram_mb: Auto::Auto,
            ram_mb: Auto::Auto,
            desktop_reserve_mb: None,
            gaming_mode: Switch::Auto,
            battery_mode: Switch::Auto,
            tick: Duration::from_secs(30),
        }
    }
}

mod duration_text {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let text = String::deserialize(d)?;
        super::parse_duration(&text).ok_or_else(|| {
            serde::de::Error::custom(format!("niepoprawny czas `{text}` (np. 30s, 10m)"))
        })
    }
}

/// Czas w formacie `"500ms"`, `"30s"`, `"10m"`, `"1h"`.
pub fn parse_duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    let split = text.find(|c: char| !c.is_ascii_digit())?;
    let (num, unit) = text.split_at(split);
    let n: u64 = num.parse().ok()?;
    match unit {
        "ms" => Some(Duration::from_millis(n)),
        "s" => Some(Duration::from_secs(n)),
        "m" => Some(Duration::from_secs(n.checked_mul(60)?)),
        "h" => Some(Duration::from_secs(n.checked_mul(3_600)?)),
        _ => None,
    }
}

/// Sygnały z maską konfiguracji (`Off` → sygnał ignorowany).
pub struct Masked<S> {
    inner: S,
    gaming: bool,
    battery: bool,
}

impl<S: ModeSource> ModeSource for Masked<S> {
    fn fullscreen_active(&self) -> bool {
        self.gaming && self.inner.fullscreen_active()
    }

    fn on_battery(&self) -> bool {
        self.battery && self.inner.on_battery()
    }
}

impl ResidencyConfig {
    /// Parsuje sekcję `[machine.residency]` (treść sekcji jako TOML).
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("[machine.residency]: {e}"))
    }

    /// Budżet: rekomendacja `device-profile` z nadpisaniami.
    pub fn budget(&self, rec: &ResidencyBudget) -> Budget {
        let reserve = self.desktop_reserve_mb.unwrap_or(rec.desktop_reserve_mb);
        let physical = rec.vram_mb.saturating_add(rec.desktop_reserve_mb);
        Budget {
            vram_mb: match self.vram_mb {
                Auto::Auto => physical.saturating_sub(reserve),
                Auto::Mb(v) => v,
            },
            ram_mb: match self.ram_mb {
                Auto::Auto => rec.ram_mb,
                Auto::Mb(v) => v,
            },
            desktop_reserve_mb: reserve,
            stt_tts_exclusive: rec.stt_tts_exclusive,
        }
    }

    /// Sygnały trybu z uwzględnieniem przełączników.
    pub fn signals<S: ModeSource>(&self, inner: S) -> Masked<S> {
        Masked {
            inner,
            gaming: self.gaming_mode == Switch::Auto,
            battery: self.battery_mode == Switch::Auto,
        }
    }
}
