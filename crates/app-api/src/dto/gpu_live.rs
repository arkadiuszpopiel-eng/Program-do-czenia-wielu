//! Karty graficzne na żywo (`device_gpu_live`, strona „Urządzenia”): pamięć, obciążenie,
//! temperatura i moc każdej karty oraz programy, które z kart korzystają.

use serde::{Deserialize, Serialize};

/// Stan jednej karty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GpuLiveCard {
    pub index: u32,
    pub name: String,
    /// Pamięć razem (MiB).
    pub total_mb: u32,
    /// Pamięć zajęta (MiB).
    pub used_mb: u32,
    /// Pamięć wolna (MiB).
    pub free_mb: u32,
    /// Obciążenie (%); `None` — karta go nie podaje.
    pub load_pct: Option<u32>,
    /// Temperatura (°C).
    pub temp_c: Option<u32>,
    /// Pobór mocy (W).
    pub power_w: Option<u32>,
}

/// Program korzystający z karty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GpuLiveProcess {
    pub pid: u32,
    /// Nazwa pliku programu (bez katalogu).
    pub name: String,
    /// Zajęta pamięć karty (MiB); `None` — system jej nie podaje.
    pub used_mb: Option<u32>,
    /// Program uruchomiony przez Alfę (silnik modelu, mowy albo sam program).
    pub alfa: bool,
}

/// Odczyt kart na żywo.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GpuLive {
    /// Pomiar dostępny (dziś: karty NVIDIA ze sterownikiem); `false` — listy są puste.
    pub available: bool,
    pub cards: Vec<GpuLiveCard>,
    pub processes: Vec<GpuLiveProcess>,
}
