//! Wspólne DTO (odpowiedniki `types.ts`): pieniądze, tekst zlokalizowany, poziomy i profile.

use serde::{Deserialize, Serialize};

/// Data i czas RFC 3339 (UTC) w formacie `toISOString()` (milisekundy, `Z`).
pub type Iso8601 = String;

/// Formatuje czas jak `Date.prototype.toISOString()` (np. `2026-09-30T08:45:00.000Z`).
pub fn iso(ts: chrono::DateTime<chrono::Utc>) -> Iso8601 {
    ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Waluta kwoty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    #[serde(rename = "PLN")]
    Pln,
    #[serde(rename = "USD")]
    Usd,
}

/// Kwota w jednostkach drobnych (grosze / centy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Money {
    pub minor: i64,
    pub currency: Currency,
}

impl Money {
    pub fn pln(minor: i64) -> Self {
        Self {
            minor,
            currency: Currency::Pln,
        }
    }

    pub fn from_micro_pln(micro: u64) -> Self {
        let grosze = cost_meter_contract::micro_pln_to_grosze(micro);
        Self::pln(i64::try_from(grosze).unwrap_or(i64::MAX))
    }
}

/// Tekst w dwóch językach interfejsu.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalizedText {
    pub pl: String,
    pub en: String,
}

impl LocalizedText {
    pub fn new(pl: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            pl: pl.into(),
            en: en.into(),
        }
    }
}

/// Język interfejsu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    Pl,
    En,
}

/// Poziom autonomii (PLAN §8.3).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum AutonomyLevel {
    L0,
    L1,
    L2,
    #[default]
    L3,
    L4,
}

/// Profil modelu sesji.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelProfile {
    Local,
    #[default]
    Hybrid,
    Cloud,
}

impl ModelProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Hybrid => "hybrid",
            Self::Cloud => "cloud",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "local" => Some(Self::Local),
            "hybrid" => Some(Self::Hybrid),
            "cloud" => Some(Self::Cloud),
            _ => None,
        }
    }
}

/// Deserializacja szerokości w pikselach: przyjmuje liczbę całkowitą albo ułamkową (zaokrąglaną).
pub(crate) mod px {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
        let v = f64::deserialize(d)?;
        if !v.is_finite() || v < 0.0 {
            return Err(serde::de::Error::custom("szerokość musi być liczbą ≥ 0"));
        }
        let rounded = v.round().min(f64::from(u32::MAX));
        // Zakres sprawdzony wyżej; konwersja bez utraty dla wartości całkowitych ≤ u32::MAX.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(rounded as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_matches_js_to_iso_string() {
        let ts = chrono::DateTime::parse_from_rfc3339("2026-09-30T08:45:00Z")
            .map(|t| t.with_timezone(&chrono::Utc));
        assert_eq!(
            ts.map(iso).ok().as_deref(),
            Some("2026-09-30T08:45:00.000Z")
        );
    }

    #[test]
    fn money_and_levels_serialize_like_ts() {
        let json = serde_json::to_value(Money::pln(214)).ok();
        assert_eq!(
            json,
            Some(serde_json::json!({"minor": 214, "currency": "PLN"}))
        );
        assert_eq!(
            serde_json::to_value(AutonomyLevel::L3).ok(),
            Some(serde_json::json!("L3"))
        );
        assert_eq!(ModelProfile::parse("cloud"), Some(ModelProfile::Cloud));
    }
}
