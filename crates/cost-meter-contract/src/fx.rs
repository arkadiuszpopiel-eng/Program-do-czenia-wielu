//! Kurs USD→PLN: NBP tabela A (raz dziennie), kurs zapasowy z konfiguracji, cache dzienny.

use async_trait::async_trait;
use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::money::RATE_SCALE;

/// Endpoint NBP: średni kurs USD z tabeli A (JSON).
pub const NBP_USD_URL: &str = "https://api.nbp.pl/api/exchangerates/rates/a/usd/?format=json";

/// Domyślny kurs zapasowy: 4,0000 PLN/USD (SPEC: `fx.fallback_usd_pln = 4.0`).
pub const DEFAULT_FALLBACK_RATE_E4: u32 = 40_000;

/// Dopuszczalny zakres kursu (ochrona przed błędną odpowiedzią): 0,5–50 PLN/USD.
pub const RATE_SANITY_E4: std::ops::RangeInclusive<u32> = 5_000..=500_000;

/// Pochodzenie kursu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FxOrigin {
    /// NBP, tabela A.
    Nbp,
    /// Kurs zapasowy z konfiguracji.
    Fallback,
}

/// Kurs użyty do przeliczenia (zapisywany w każdym rekordzie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FxRate {
    /// PLN za 1 USD × 10⁴ (3,6512 → 36 512).
    pub rate_e4: u32,
    /// Data notowania NBP (brak dla kursu zapasowego).
    pub effective_date: Option<NaiveDate>,
    /// Pochodzenie.
    pub origin: FxOrigin,
    /// Czy kurs nie został odświeżony dzisiaj.
    pub stale: bool,
}

/// Notowanie pobrane ze źródła.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FxQuote {
    /// PLN za 1 USD × 10⁴.
    pub rate_e4: u32,
    /// Data notowania.
    pub effective_date: NaiveDate,
}

/// Błędy pobierania kursu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum FxError {
    /// Brak sieci / błąd HTTP.
    #[error("nie można pobrać kursu: {0}")]
    Network(String),
    /// Odpowiedź w nieoczekiwanym formacie.
    #[error("niepoprawna odpowiedź kursu: {0}")]
    Parse(String),
}

/// Źródło kursu (produkcyjnie NBP przez `net.egress(api.nbp.pl)`).
#[async_trait]
pub trait FxSource: Send + Sync {
    /// Aktualny średni kurs USD/PLN.
    async fn fetch_usd_pln(&self) -> Result<FxQuote, FxError>;
}

/// Liczba dziesiętna (`3.6512`) → ×10⁴ bez `f64`; więcej niż 4 miejsca — zaokrąglenie połowa w górę.
pub fn parse_decimal_e4(text: &str) -> Option<u32> {
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let (int, frac) = match text.split_once('.') {
        Some((int, frac)) if digits(frac) => (int, frac),
        Some(_) => return None,
        None => (text, ""),
    };
    if !digits(int) {
        return None;
    }
    let int: u64 = int.parse().ok()?;
    let mut frac_e4: u64 = 0;
    for (i, c) in frac.chars().take(4).enumerate() {
        let d = u64::from(c.to_digit(10)?);
        frac_e4 += d * 10u64.pow(3 - u32::try_from(i).ok()?);
    }
    let round_up = frac.chars().nth(4).is_some_and(|c| c >= '5');
    let value = int
        .checked_mul(RATE_SCALE)?
        .checked_add(frac_e4 + u64::from(round_up))?;
    u32::try_from(value).ok()
}

#[derive(Deserialize)]
struct NbpResponse {
    code: String,
    rates: Vec<NbpRate>,
}

#[derive(Deserialize)]
struct NbpRate {
    #[serde(rename = "effectiveDate")]
    effective_date: NaiveDate,
    mid: serde_json::Number,
}

/// Parsuje odpowiedź NBP (`/exchangerates/rates/a/usd/?format=json`); bierze ostatnie notowanie.
pub fn parse_nbp_json(text: &str) -> Result<FxQuote, FxError> {
    let resp: NbpResponse =
        serde_json::from_str(text).map_err(|e| FxError::Parse(e.to_string()))?;
    if !resp.code.eq_ignore_ascii_case("USD") {
        return Err(FxError::Parse(format!("waluta {} zamiast USD", resp.code)));
    }
    let rate = resp
        .rates
        .last()
        .ok_or_else(|| FxError::Parse("brak notowań".into()))?;
    let rate_e4 = parse_decimal_e4(&rate.mid.to_string())
        .filter(|r| RATE_SANITY_E4.contains(r))
        .ok_or_else(|| FxError::Parse(format!("kurs poza zakresem: {}", rate.mid)))?;
    Ok(FxQuote {
        rate_e4,
        effective_date: rate.effective_date,
    })
}

/// Cache kursu: odświeżany raz dziennie; bez notowania — kurs zapasowy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxCache {
    last: Option<(FxQuote, NaiveDate)>,
    fallback_e4: u32,
}

impl FxCache {
    /// Pusty cache z kursem zapasowym.
    pub fn new(fallback_e4: u32) -> Self {
        Self {
            last: None,
            fallback_e4,
        }
    }

    /// Zmienia kurs zapasowy.
    pub fn set_fallback(&mut self, fallback_e4: u32) {
        self.fallback_e4 = fallback_e4;
    }

    /// Czy trzeba pobrać kurs (nie pobrano dzisiaj).
    pub fn needs_refresh(&self, today: NaiveDate) -> bool {
        self.last.is_none_or(|(_, fetched)| fetched != today)
    }

    /// Zapamiętuje notowanie pobrane w dniu `fetched_on`.
    pub fn store(&mut self, quote: FxQuote, fetched_on: NaiveDate) {
        self.last = Some((quote, fetched_on));
    }

    /// Kurs na dziś: ostatnie notowanie (oznaczone `stale`, jeśli nie z dziś) albo zapasowy.
    pub fn current(&self, today: NaiveDate) -> FxRate {
        match self.last {
            Some((q, fetched)) => FxRate {
                rate_e4: q.rate_e4,
                effective_date: Some(q.effective_date),
                origin: FxOrigin::Nbp,
                stale: fetched != today,
            },
            None => FxRate {
                rate_e4: self.fallback_e4,
                effective_date: None,
                origin: FxOrigin::Fallback,
                stale: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_without_floats() {
        assert_eq!(parse_decimal_e4("3.6512"), Some(36_512));
        assert_eq!(parse_decimal_e4("4"), Some(40_000));
        assert_eq!(parse_decimal_e4("3.65"), Some(36_500));
        assert_eq!(parse_decimal_e4("3.65125"), Some(36_513));
        assert_eq!(parse_decimal_e4("3.65124"), Some(36_512));
        for bad in ["", ".5", "3.", "-1", "1e3", "3,65", "abc"] {
            assert_eq!(parse_decimal_e4(bad), None, "{bad}");
        }
    }

    #[test]
    fn nbp_response() {
        let ok = r#"{"table":"A","currency":"dolar amerykański","code":"USD",
            "rates":[{"no":"188/A/NBP/2026","effectiveDate":"2026-09-29","mid":3.6512}]}"#;
        let q = parse_nbp_json(ok).unwrap();
        assert_eq!(q.rate_e4, 36_512);
        assert_eq!(
            q.effective_date,
            NaiveDate::from_ymd_opt(2026, 9, 29).unwrap()
        );
        assert!(parse_nbp_json(&ok.replace("USD", "EUR")).is_err());
        assert!(parse_nbp_json(&ok.replace("3.6512", "0.0001")).is_err());
        assert!(parse_nbp_json(r#"{"code":"USD","rates":[]}"#).is_err());
        assert!(parse_nbp_json("<html>").is_err());
    }

    #[test]
    fn cache_daily() {
        let d1 = NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let mut cache = FxCache::new(DEFAULT_FALLBACK_RATE_E4);
        assert!(cache.needs_refresh(d1));
        let fallback = cache.current(d1);
        assert_eq!(
            (fallback.rate_e4, fallback.origin),
            (40_000, FxOrigin::Fallback)
        );
        cache.store(
            FxQuote {
                rate_e4: 36_512,
                effective_date: d1,
            },
            d1,
        );
        assert!(!cache.needs_refresh(d1));
        assert!(!cache.current(d1).stale);
        assert!(cache.needs_refresh(d2));
        assert!(cache.current(d2).stale);
        assert_eq!(cache.current(d2).rate_e4, 36_512);
    }
}
