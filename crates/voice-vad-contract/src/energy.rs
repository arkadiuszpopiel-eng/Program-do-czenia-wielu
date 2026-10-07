//! Detektor energetyczny (deterministyczny): prawdopodobieństwo mowy z poziomu ramki względem
//! szumu tła. Silnik atrapy i zapas `-impl`, gdy model Silero jest niedostępny.

use voice_audio_contract::gain::rms_db;
use voice_dsp_contract::NoiseFloorTracker;

/// Ile dB ponad szum oznacza p = 0,5.
const MARGIN_DB: f32 = 9.0;
/// Nachylenie (dB na jednostkę logitu).
const SLOPE_DB: f32 = 2.5;
/// Poniżej tego poziomu zawsze cisza (dBFS).
const ABS_FLOOR_DB: f32 = -60.0;

/// Detektor mowy z energii.
#[derive(Debug, Clone)]
pub struct EnergyDetector {
    tracker: NoiseFloorTracker,
    external_floor: Option<f32>,
}

impl Default for EnergyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl EnergyDetector {
    /// Nowy detektor (ramki 10 ms do śledzenia szumu).
    pub fn new() -> Self {
        Self {
            tracker: NoiseFloorTracker::new(10),
            external_floor: None,
        }
    }

    /// Szum podany z zewnątrz (`voice-dsp`) zamiast własnego śledzenia.
    pub fn set_noise_floor(&mut self, db: f32) {
        self.external_floor = Some(db);
    }

    /// Prawdopodobieństwo mowy w ramce.
    pub fn prob(&mut self, samples: &[f32]) -> f32 {
        let level = rms_db(samples);
        let own = self.tracker.update(level);
        let floor = self.external_floor.unwrap_or(own).max(own.min(-90.0));
        if level < ABS_FLOOR_DB {
            return 0.0;
        }
        let x = (level - floor - MARGIN_DB) / SLOPE_DB;
        1.0 / (1.0 + (-x).exp())
    }

    /// Reset.
    pub fn reset(&mut self) {
        self.tracker.reset();
        self.external_floor = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::synth::{sine, white_noise};

    #[test]
    fn speech_above_noise_is_detected() {
        let mut d = EnergyDetector::new();
        let noise = white_noise(1, 160, 0.003);
        for _ in 0..50 {
            assert!(d.prob(&noise) < 0.2);
        }
        let tone: Vec<f32> = sine(200.0, 16_000, 0.01, 0.1)
            .iter()
            .zip(&noise)
            .map(|(a, b)| a + b)
            .collect();
        assert!(d.prob(&tone) > 0.9);
        assert_eq!(d.prob(&vec![0.0; 160]), 0.0);
        d.set_noise_floor(-20.0);
        assert!(d.prob(&tone) < 0.5);
        d.reset();
    }
}
