//! Koszt wywołań liczony z tabeli cen z konfiguracji (nigdy z kodu, PLAN §5.5).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::event::Usage;
use crate::message::{ContentBlock, ImageSource, ToolResultPart};
use crate::request::ChatRequest;

/// Cennik modelu w USD za milion tokenów (klucze jak w `[providers.api.<id>.pricing]`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pricing {
    /// Wejście (bez cache).
    pub input_per_mtok_usd: f64,
    /// Wyjście.
    pub output_per_mtok_usd: f64,
    /// Odczyt z cache; brak → liczony jak wejście (górne oszacowanie).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_per_mtok_usd: Option<f64>,
    /// Zapis do cache; brak → liczony jak wejście.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_per_mtok_usd: Option<f64>,
}

/// Tabela cen: model → cennik.
pub type PricingTable = BTreeMap<String, Pricing>;

/// Koszt w nano-USD (1 USD = 10⁹) — dokładny dla cen z ≤ 3 miejscami po przecinku za MTok.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub struct Cost {
    /// Nano-dolary.
    pub nano_usd: u64,
}

impl Cost {
    /// Koszt w USD (do wyświetlania).
    #[allow(clippy::cast_precision_loss)]
    pub fn usd(self) -> f64 {
        self.nano_usd as f64 / 1e9
    }

    /// Koszt w mikro-USD zaokrąglony w górę (format `core-bus-contract::Cost`).
    pub fn micro_usd_ceil(self) -> u64 {
        self.nano_usd.div_ceil(1000)
    }
}

impl std::ops::Add for Cost {
    type Output = Cost;
    fn add(self, rhs: Cost) -> Cost {
        Cost {
            nano_usd: self.nano_usd.saturating_add(rhs.nano_usd),
        }
    }
}

/// Wstępne oszacowanie kosztu żądania (przed wywołaniem; dla budżetów Routera).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CostEstimate {
    /// Szacowane tokeny wejścia (heurystyka: znaki / 4, obrazy stałą wagą).
    pub input_tokens: u64,
    /// Górny limit tokenów wyjścia (`max_tokens` po rozstrzygnięciu).
    pub max_output_tokens: u64,
    /// Koszt minimalny (samo wejście, bez cache).
    pub min: Cost,
    /// Koszt maksymalny (wejście + pełne `max_tokens`).
    pub max: Cost,
}

/// Cena za token w nano-USD (USD/MTok × 1000), zaokrąglona do najbliższej wartości.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn nano_per_token(per_mtok_usd: f64) -> u64 {
    if per_mtok_usd.is_finite() && per_mtok_usd > 0.0 {
        (per_mtok_usd * 1000.0).round() as u64
    } else {
        0
    }
}

impl Pricing {
    /// Koszt zużycia.
    ///
    /// ```
    /// use providers_contract::{Pricing, Usage};
    /// let p = Pricing { input_per_mtok_usd: 4.0, output_per_mtok_usd: 20.0,
    ///                   cache_read_per_mtok_usd: Some(0.2), cache_write_per_mtok_usd: Some(5.0) };
    /// let u = Usage { input_tokens: 1_000_000, output_tokens: 100_000,
    ///                 cache_read_tokens: 1_000_000, cache_write_tokens: 0 };
    /// // 4,00 $ (wejście) + 2,00 $ (wyjście) + 0,20 $ (odczyt cache)
    /// assert_eq!(p.cost(&u).nano_usd, 6_200_000_000);
    /// ```
    pub fn cost(&self, usage: &Usage) -> Cost {
        let input = nano_per_token(self.input_per_mtok_usd);
        let read = self.cache_read_per_mtok_usd.map_or(input, nano_per_token);
        let write = self.cache_write_per_mtok_usd.map_or(input, nano_per_token);
        let parts = [
            usage.input_tokens.saturating_mul(input),
            usage
                .output_tokens
                .saturating_mul(nano_per_token(self.output_per_mtok_usd)),
            usage.cache_read_tokens.saturating_mul(read),
            usage.cache_write_tokens.saturating_mul(write),
        ];
        Cost {
            nano_usd: parts.iter().fold(0u64, |acc, p| acc.saturating_add(*p)),
        }
    }

    /// Oszacowanie kosztu żądania przy danym limicie wyjścia.
    pub fn estimate(&self, req: &ChatRequest, max_output_tokens: u64) -> CostEstimate {
        let input_tokens = estimate_input_tokens(req);
        let min = self.cost(&Usage {
            input_tokens,
            ..Usage::default()
        });
        let max = self.cost(&Usage {
            input_tokens,
            output_tokens: max_output_tokens,
            ..Usage::default()
        });
        CostEstimate {
            input_tokens,
            max_output_tokens,
            min,
            max,
        }
    }
}

/// Umowna waga obrazu w tokenach (heurystyka oszacowania; dokładne liczenie robi dostawca).
pub const IMAGE_TOKEN_ESTIMATE: u64 = 1_600;

/// Heurystyczna liczba tokenów wejścia: ⌈znaki / 4⌉ tekstu + narzędzia + obrazy.
pub fn estimate_input_tokens(req: &ChatRequest) -> u64 {
    let mut chars = req.system.as_ref().map_or(0, |s| s.chars().count());
    let mut images = 0u64;
    for tool in &req.tools {
        chars += tool.name.len() + tool.description.chars().count();
        chars += tool.input_schema.to_string().len();
    }
    for msg in &req.messages {
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => chars += text.chars().count(),
                ContentBlock::Thinking(t) => chars += t.text.chars().count(),
                ContentBlock::RedactedThinking(r) => chars += r.data.len() / 4,
                ContentBlock::ToolUse(t) => chars += t.name.len() + t.input.to_string().len(),
                ContentBlock::ToolResult(r) => {
                    for part in &r.content {
                        match part {
                            ToolResultPart::Text { text } => chars += text.chars().count(),
                            ToolResultPart::Image { .. } => images += 1,
                        }
                    }
                }
                ContentBlock::Image { source } => {
                    images += 1;
                    if let ImageSource::Url { url } = source {
                        chars += url.len();
                    }
                }
            }
        }
    }
    (chars as u64).div_ceil(4) + images * IMAGE_TOKEN_ESTIMATE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    fn opus_like() -> Pricing {
        Pricing {
            input_per_mtok_usd: 4.0,
            output_per_mtok_usd: 20.0,
            cache_read_per_mtok_usd: None,
            cache_write_per_mtok_usd: None,
        }
    }

    #[test]
    fn cache_without_price_counts_as_input() {
        let usage = Usage {
            cache_read_tokens: 1_000,
            ..Usage::default()
        };
        assert_eq!(opus_like().cost(&usage).nano_usd, 4_000_000);
    }

    #[test]
    fn cheap_prices_are_exact() {
        let p = Pricing {
            input_per_mtok_usd: 0.14,
            output_per_mtok_usd: 0.42,
            cache_read_per_mtok_usd: None,
            cache_write_per_mtok_usd: None,
        };
        let c = p.cost(&Usage {
            input_tokens: 1,
            output_tokens: 1,
            ..Usage::default()
        });
        assert_eq!(c.nano_usd, 560);
        assert_eq!(c.micro_usd_ceil(), 1);
        assert_eq!((c + c).nano_usd, 1_120);
    }

    #[test]
    fn estimate_bounds() {
        let req = ChatRequest::new("m", vec![Message::user_text("x".repeat(400))]);
        let est = opus_like().estimate(&req, 1_000);
        assert_eq!(est.input_tokens, 100);
        assert!(est.max > est.min);
        assert_eq!(est.max.nano_usd, 100 * 4_000 + 1_000 * 20_000);
    }

    #[test]
    fn invalid_prices_are_zero() {
        assert_eq!(nano_per_token(f64::NAN), 0);
        assert_eq!(nano_per_token(-1.0), 0);
    }
}
