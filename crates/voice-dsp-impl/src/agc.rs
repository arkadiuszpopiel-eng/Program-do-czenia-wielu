//! Prosty AGC: poziom RMS ramek z mową → wzmocnienie dążące do celu (szybko w dół, wolno w górę),
//! interpolowane w obrębie ramki (bez „schodków”), z ogranicznikiem szczytu.

use voice_audio_contract::gain::{db_to_gain, peak, rms_db};

/// Automatyczna regulacja wzmocnienia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Agc {
    /// Docelowy poziom mowy (dBFS RMS).
    pub target_db: f32,
    /// Maksymalne wzmocnienie (dB).
    pub max_gain_db: f32,
    /// Minimalne wzmocnienie (dB).
    pub min_gain_db: f32,
    gain_db: f32,
}

/// Sufit szczytowy po wzmocnieniu (−1 dBFS).
const CEILING: f32 = 0.89;

impl Agc {
    /// AGC z celem `target_db`; tryb szeptu dopuszcza +30 dB zamiast +20 dB.
    pub fn new(target_db: f32, whisper_mode: bool) -> Self {
        Self {
            target_db,
            max_gain_db: if whisper_mode { 30.0 } else { 20.0 },
            min_gain_db: -20.0,
            gain_db: 0.0,
        }
    }

    /// Bieżące wzmocnienie (dB).
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    /// Przetwarza ramkę w miejscu; `speech` = ramka zawiera mowę (tylko wtedy wzmocnienie się zmienia).
    pub fn process(&mut self, block: &mut [f32], speech: bool) {
        let from = self.gain_db;
        let level = rms_db(block);
        if speech && level > -75.0 {
            let desired = (self.target_db - level).clamp(self.min_gain_db, self.max_gain_db);
            let k = if desired < self.gain_db { 0.3 } else { 0.05 };
            self.gain_db += (desired - self.gain_db) * k;
        }
        let (g0, g1) = (db_to_gain(from), db_to_gain(self.gain_db));
        let n = block.len().max(1) as f32;
        for (i, s) in block.iter_mut().enumerate() {
            *s *= g0 + (g1 - g0) * (i as f32 / n);
        }
        let p = peak(block);
        if p > CEILING {
            let k = CEILING / p;
            for s in block.iter_mut() {
                *s *= k;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::gain::db_to_gain;
    use voice_audio_contract::synth::{sine, white_noise};

    fn run(agc: &mut Agc, level_db: f32, secs: usize, speech: bool) -> f32 {
        let amp = db_to_gain(level_db + 3.01);
        let mut last = 0.0;
        for _ in 0..secs * 100 {
            let mut b = sine(220.0, 48_000, 0.01, amp);
            agc.process(&mut b, speech);
            last = rms_db(&b);
        }
        last
    }

    #[test]
    fn converges_to_target_from_quiet_and_loud() {
        for level in [-35.0f32, -28.0, -10.0, -5.0] {
            let mut agc = Agc::new(-20.0, false);
            let out = run(&mut agc, level, 3, true);
            assert!((out + 20.0).abs() < 2.0, "{level} dB → {out} dB");
        }
    }

    #[test]
    fn gain_is_bounded_and_whisper_mode_allows_more() {
        let mut normal = Agc::new(-20.0, false);
        let mut whisper = Agc::new(-20.0, true);
        let a = run(&mut normal, -48.0, 4, true);
        let b = run(&mut whisper, -48.0, 4, true);
        assert!((a + 28.0).abs() < 1.0, "{a}");
        assert!((b + 20.0).abs() < 2.0, "{b}");
        assert!(normal.gain_db() <= 20.0 + 1e-3);
    }

    #[test]
    fn noise_without_speech_does_not_pump_and_peaks_are_limited() {
        let mut agc = Agc::new(-20.0, false);
        let mut n = white_noise(4, 480, 0.001);
        for _ in 0..300 {
            agc.process(&mut n, false);
        }
        assert_eq!(agc.gain_db(), 0.0);
        let mut loud = Agc::new(-3.0, false);
        let mut b = sine(220.0, 48_000, 0.01, 0.99);
        for _ in 0..50 {
            loud.process(&mut b, true);
            assert!(peak(&b) <= CEILING + 1e-5);
        }
    }
}
