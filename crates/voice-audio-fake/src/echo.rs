//! Ścieżka echa głośnik → mikrofon (odpowiedź impulsowa pokoju) dla testów AEC i barge-in.

use std::time::Duration;

use voice_audio_contract::synth::Rng;

/// Echo: opóźnienie akustyczne + filtr FIR (odpowiedź pokoju) + tłumienie.
#[derive(Debug, Clone, PartialEq)]
pub struct EchoPath {
    /// Opóźnienie od odtworzenia na urządzeniu do mikrofonu.
    pub delay: Duration,
    /// Współczynniki FIR (w próbkach urządzenia).
    pub taps: Vec<f32>,
}

impl EchoPath {
    /// Samo opóźnienie i wzmocnienie (bez pogłosu).
    pub fn simple(delay: Duration, gain: f32) -> Self {
        Self {
            delay,
            taps: vec![gain],
        }
    }

    /// „Pokój”: tor bezpośredni + wykładniczo gasnące odbicia (`rt_ms` do −60 dB), deterministyczne.
    pub fn room(delay: Duration, gain: f32, rt_ms: u32, rate: u32, seed: u64) -> Self {
        let len = (rate as usize * rt_ms as usize / 1000).max(1);
        let mut rng = Rng::new(seed);
        let decay = (-6.9 / len as f32).exp(); // e^{-6.9} ≈ −60 dB na końcu
        let mut taps = Vec::with_capacity(len);
        let mut env = 1.0f32;
        for i in 0..len {
            let v = if i == 0 {
                1.0
            } else {
                0.35 * rng.gaussian() * env
            };
            taps.push(v);
            env *= decay;
        }
        let energy: f32 = taps.iter().map(|t| t * t).sum::<f32>().sqrt();
        for t in &mut taps {
            *t *= gain / energy;
        }
        Self { delay, taps }
    }

    /// Opóźnienie w próbkach.
    pub fn delay_samples(&self, rate: u32) -> usize {
        (self.delay.as_nanos() * u128::from(rate) / 1_000_000_000) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_is_normalized_and_deterministic() {
        let a = EchoPath::room(Duration::from_millis(30), 0.5, 50, 16_000, 3);
        let b = EchoPath::room(Duration::from_millis(30), 0.5, 50, 16_000, 3);
        assert_eq!(a, b);
        assert_eq!(a.taps.len(), 800);
        let norm: f32 = a.taps.iter().map(|t| t * t).sum::<f32>().sqrt();
        assert!((norm - 0.5).abs() < 1e-4);
        assert_eq!(a.delay_samples(16_000), 480);
        assert_eq!(EchoPath::simple(Duration::ZERO, 0.3).taps, vec![0.3]);
    }
}
