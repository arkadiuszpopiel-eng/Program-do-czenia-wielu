//! Statystyka bez zależności zewnętrznych: średnia, bootstrap percentylowy (także sparowany)
//! z deterministycznym generatorem SplitMix64 — ten sam wynik przy tym samym ziarnie.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Generator SplitMix64 (deterministyczny, do bootstrapu i atrap).
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Generator z ziarnem.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Następna liczba 64-bitowa.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Liczba z `0..n` (`n` > 0; dla 0 zwraca 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        let n64 = u64::try_from(n).unwrap_or(u64::MAX);
        usize::try_from(self.next_u64() % n64).unwrap_or(0)
    }
}

/// Wartość punktowa z przedziałem ufności.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Interval {
    /// Średnia.
    pub mean: f64,
    /// Dolna granica.
    pub lo: f64,
    /// Górna granica.
    pub hi: f64,
}

impl Interval {
    /// Przedział zdegenerowany (jedna obserwacja).
    pub fn point(value: f64) -> Self {
        Self {
            mean: value,
            lo: value,
            hi: value,
        }
    }

    /// Zaokrąglenie do `decimals` miejsc (wynik zbiorczy holdoutu — mniej informacji).
    #[must_use]
    pub fn rounded(self, decimals: u32) -> Self {
        let f = 10f64.powi(i32::try_from(decimals.min(9)).unwrap_or(9));
        let r = |v: f64| (v * f).round() / f;
        Self {
            mean: r(self.mean),
            lo: r(self.lo),
            hi: r(self.hi),
        }
    }
}

/// Parametry bootstrapu.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BootstrapConfig {
    /// Liczba losowań.
    pub iterations: u32,
    /// Poziom ufności (np. 0,95).
    pub confidence: f64,
    /// Ziarno.
    pub seed: u64,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            iterations: 2000,
            confidence: 0.95,
            seed: 0x5EED_A1FA_F8F8_0001,
        }
    }
}

/// Średnia arytmetyczna (`None` dla pustego wejścia).
pub fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    Some(xs.iter().sum::<f64>() / xs.len() as f64)
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    let last = sorted.len().saturating_sub(1);
    let pos = (q.clamp(0.0, 1.0) * last as f64).round();
    let idx = if pos.is_finite() && pos > 0.0 {
        usize::try_from(pos as u64).unwrap_or(last).min(last)
    } else {
        0
    };
    sorted.get(idx).copied().unwrap_or(0.0)
}

fn resample_means(xs: &[f64], cfg: &BootstrapConfig) -> Vec<f64> {
    let mut rng = SplitMix64::new(cfg.seed);
    let n = xs.len();
    let mut means: Vec<f64> = (0..cfg.iterations.max(1))
        .map(|_| (0..n).map(|_| xs[rng.below(n)]).sum::<f64>() / n as f64)
        .collect();
    means.sort_by(f64::total_cmp);
    means
}

/// Bootstrap percentylowy średniej. Jedna obserwacja → przedział punktowy.
pub fn bootstrap_mean(xs: &[f64], cfg: &BootstrapConfig) -> Option<Interval> {
    let m = mean(xs)?;
    if xs.len() == 1 {
        return Some(Interval::point(m));
    }
    let alpha = (1.0 - cfg.confidence.clamp(0.5, 0.999)) / 2.0;
    let means = resample_means(xs, cfg);
    Some(Interval {
        mean: m,
        lo: percentile(&means, alpha),
        hi: percentile(&means, 1.0 - alpha),
    })
}

/// Bootstrap sparowanej różnicy `candidate[i] − baseline[i]` (te same przypadki).
pub fn bootstrap_paired_delta(
    baseline: &[f64],
    candidate: &[f64],
    cfg: &BootstrapConfig,
) -> Option<Interval> {
    if baseline.len() != candidate.len() {
        return None;
    }
    let deltas: Vec<f64> = baseline.iter().zip(candidate).map(|(b, c)| c - b).collect();
    bootstrap_mean(&deltas, cfg)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn splitmix_is_deterministic_and_bounded() {
        let mut a = SplitMix64::new(7);
        let mut b = SplitMix64::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
            assert!(a.below(10) < 10);
            b.below(10);
        }
        assert_eq!(a.below(0), 0);
    }

    #[test]
    fn interval_rounding_and_edge_cases() {
        let cfg = BootstrapConfig::default();
        assert_eq!(bootstrap_mean(&[], &cfg), None);
        assert_eq!(bootstrap_mean(&[0.5], &cfg), Some(Interval::point(0.5)));
        let i = Interval {
            mean: 0.123_456,
            lo: 0.1,
            hi: 0.98765,
        }
        .rounded(2);
        assert_eq!((i.mean, i.lo, i.hi), (0.12, 0.1, 0.99));
        assert_eq!(bootstrap_paired_delta(&[1.0], &[1.0, 2.0], &cfg), None);
        let d = bootstrap_paired_delta(&[0.0; 20], &[1.0; 20], &cfg).unwrap();
        assert_eq!((d.mean, d.lo, d.hi), (1.0, 1.0, 1.0));
    }

    proptest! {
        #[test]
        fn interval_brackets_mean(xs in proptest::collection::vec(0.0f64..1.0, 2..60), seed in any::<u64>()) {
            let cfg = BootstrapConfig { iterations: 300, confidence: 0.95, seed };
            let i = bootstrap_mean(&xs, &cfg).unwrap();
            let min = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            prop_assert!(i.lo <= i.hi);
            prop_assert!(i.lo >= min - 1e-12 && i.hi <= max + 1e-12);
            prop_assert_eq!(bootstrap_mean(&xs, &cfg), Some(i));
        }
    }
}
