//! Cennik modeli (Ustawienia → Cennik modeli): obowiązujące ceny i podgląd zmian przed zapisem.

use serde::{Deserialize, Serialize};

/// Cena modelu (USD za 1 mln tokenów).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PriceRow {
    pub provider: String,
    pub model: String,
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
    pub source: String,
    pub note: Option<String>,
    /// Cena z pliku użytkownika (zatwierdzona aktualizacja), nie z wydania.
    pub user: bool,
}

/// Zmiana ceny: nowa pozycja i cena sprzed zmiany (`None` — nowy model).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PriceChangeRow {
    pub after: PriceRow,
    pub before: Option<PriceRow>,
}

/// Cennik obowiązujący i — po `pricing_apply` — zmiany (podgląd albo zapisane).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PricingView {
    /// Dzień ostatniego sprawdzenia cen.
    pub checked: String,
    /// Plik cennika użytkownika.
    pub file: String,
    pub rows: Vec<PriceRow>,
    pub changes: Vec<PriceChangeRow>,
    /// Czy zmiany zapisano (`false` — tylko podgląd).
    pub saved: bool,
}
