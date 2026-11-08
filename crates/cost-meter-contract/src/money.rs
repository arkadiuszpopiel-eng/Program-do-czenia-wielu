//! Arytmetyka pieniędzy na liczbach całkowitych: mikro-USD, mikro-PLN, kurs ×10⁴.
//! Pośrednie iloczyny w `u128`, zaokrąglenie „połowa w górę”, nasycenie zamiast przepełnienia.

use accounts_hub_contract::ModelPrice;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 1 USD w mikro-USD.
pub const MICRO_PER_UNIT: u64 = 1_000_000;
/// Skala kursu: kurs 3,6512 PLN/USD zapisany jako 36 512.
pub const RATE_SCALE: u64 = 10_000;
/// 1 grosz w mikro-PLN.
pub const MICRO_PLN_PER_GROSZ: u64 = 10_000;

/// Zużycie tokenów jednego wywołania.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Usage {
    /// Tokeny wejściowe (bez cache).
    pub input_tokens: u64,
    /// Tokeny wyjściowe.
    pub output_tokens: u64,
    /// Tokeny odczytane z cache promptów.
    #[serde(default)]
    pub cache_read_tokens: u64,
    /// Tokeny zapisane do cache promptów.
    #[serde(default)]
    pub cache_write_tokens: u64,
}

fn div_round(numerator: u128, denominator: u128) -> u64 {
    if denominator == 0 {
        return 0;
    }
    u64::try_from(numerator.saturating_add(denominator / 2) / denominator).unwrap_or(u64::MAX)
}

/// Koszt w mikro-USD: Σ tokeny × cena[mikro-USD/Mtok] / 10⁶ (zaokrąglenie połowa w górę).
pub fn cost_micro_usd(usage: &Usage, price: &ModelPrice) -> u64 {
    let parts = [
        (usage.input_tokens, price.input_micro_usd_per_mtok),
        (usage.output_tokens, price.output_micro_usd_per_mtok),
        (usage.cache_read_tokens, price.cache_read_micro_usd_per_mtok),
        (
            usage.cache_write_tokens,
            price.cache_write_micro_usd_per_mtok,
        ),
    ];
    let total = parts.iter().fold(0u128, |acc, (tokens, p)| {
        acc.saturating_add(u128::from(*tokens) * u128::from(*p))
    });
    div_round(total, u128::from(MICRO_PER_UNIT))
}

/// Przeliczenie mikro-USD → mikro-PLN kursem ×10⁴.
pub fn usd_to_pln(micro_usd: u64, rate_e4: u32) -> u64 {
    div_round(
        u128::from(micro_usd) * u128::from(rate_e4),
        u128::from(RATE_SCALE),
    )
}

/// Grosze → mikro-PLN (nasycenie).
pub fn grosze_to_micro_pln(grosze: u64) -> u64 {
    grosze.saturating_mul(MICRO_PLN_PER_GROSZ)
}

/// Mikro-PLN → grosze (zaokrąglenie połowa w górę).
pub fn micro_pln_to_grosze(micro_pln: u64) -> u64 {
    div_round(u128::from(micro_pln), u128::from(MICRO_PLN_PER_GROSZ))
}

/// Tekst dla UI: `12,34 zł` (grosze po zaokrągleniu).
pub fn format_pln(micro_pln: u64) -> String {
    let grosze = micro_pln_to_grosze(micro_pln);
    format!("{},{:02} zł", grosze / 100, grosze % 100)
}

/// Procent `part/whole` zaokrąglony w dół; `whole = 0` → 0 dla `part = 0`, inaczej `u32::MAX`.
pub fn percent(part: u64, whole: u64) -> u32 {
    if whole == 0 {
        return if part == 0 { 0 } else { u32::MAX };
    }
    u32::try_from(u128::from(part) * 100 / u128::from(whole)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_and_conversion() {
        let price = ModelPrice {
            input_micro_usd_per_mtok: 3_000_000,
            output_micro_usd_per_mtok: 15_000_000,
            cache_read_micro_usd_per_mtok: 300_000,
            cache_write_micro_usd_per_mtok: 3_750_000,
        };
        let usage = Usage {
            input_tokens: 1_000,
            output_tokens: 500,
            cache_read_tokens: 10_000,
            cache_write_tokens: 0,
        };
        // 1000×3 + 500×15 + 10000×0,3 = 3000 + 7500 + 3000 = 13 500 mikro-USD
        assert_eq!(cost_micro_usd(&usage, &price), 13_500);
        assert_eq!(usd_to_pln(13_500, 36_512), 49_291);
        assert_eq!(usd_to_pln(1, 36_512), 4);
        assert_eq!(format_pln(49_291), "0,05 zł");
        assert_eq!(format_pln(123_456_789), "123,46 zł");
        assert_eq!(grosze_to_micro_pln(10_000), 100_000_000);
        assert_eq!(percent(80, 100), 80);
        assert_eq!(percent(1, 0), u32::MAX);
        assert_eq!(percent(0, 0), 0);
    }

    #[test]
    fn saturates_instead_of_overflowing() {
        let price = ModelPrice {
            input_micro_usd_per_mtok: u64::MAX,
            ..ModelPrice::default()
        };
        let price = ModelPrice {
            output_micro_usd_per_mtok: u64::MAX,
            cache_read_micro_usd_per_mtok: u64::MAX,
            ..price
        };
        let usage = Usage {
            input_tokens: u64::MAX,
            output_tokens: u64::MAX,
            cache_read_tokens: u64::MAX,
            cache_write_tokens: 0,
        };
        assert_eq!(cost_micro_usd(&usage, &price), u64::MAX);
        assert_eq!(usd_to_pln(u64::MAX, u32::MAX), u64::MAX);
    }
}
