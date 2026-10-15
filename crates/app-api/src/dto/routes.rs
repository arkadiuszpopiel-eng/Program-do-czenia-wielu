//! DTO widoku „Gdzie trafiają dane” (Ustawienia → Dobór modelu): trasy z rejestru zgodności
//! z jurysdykcją, tagami prywatności i decyzją dla sesji prywatnej — `types-routes.ts`.
//! Tylko do odczytu: polityka i rejestr należą do Jądra.

use serde::{Deserialize, Serialize};

/// Trasa do dostawcy (API albo most CLI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DataRoute {
    pub id: String,
    pub provider: String,
    /// `api`, `cli-p` albo `sdk`.
    pub mode: String,
    /// Kody jurysdykcji (np. `US`, `EU`, `CN`); pusta lista — nieznana.
    pub jurisdiction: Vec<String>,
    /// Tagi prywatności (np. `eu`, `cn-may-train`, `unknown`).
    pub privacy: Vec<String>,
    /// Ryzyko trenowania na danych: `no_train`, `may_train`, `unknown`.
    pub training: String,
    /// Status po degradacji: `green`, `grey`, `forbidden`.
    pub status: String,
    /// Wpis rejestru nieświeży.
    pub stale: bool,
    /// Wpis katalogu niezweryfikowany.
    pub unverified: bool,
    pub enabled: bool,
    /// Czy trasa obsłuży zwykłą sesję.
    pub standard_allowed: bool,
    /// Czy trasa obsłuży sesję prywatną.
    pub private_allowed: bool,
    /// Powód decyzji dla sesji prywatnej (po polsku).
    pub private_reason: String,
}

/// Wszystkie trasy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DataRoutesView {
    pub routes: Vec<DataRoute>,
}
