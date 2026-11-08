//! Poziomy i wzmocnienie: dB ↔ liniowo, RMS, normalizacja głośności wypowiedzi.

/// Najniższy raportowany poziom (cisza cyfrowa).
pub const SILENCE_DB: f32 = -120.0;

/// dB → wzmocnienie liniowe.
pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Wzmocnienie liniowe → dB (z podłogą [`SILENCE_DB`]).
pub fn gain_to_db(gain: f32) -> f32 {
    if gain <= 0.0 {
        SILENCE_DB
    } else {
        (20.0 * gain.log10()).max(SILENCE_DB)
    }
}

/// Średnia kwadratowa.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// Poziom RMS w dBFS.
pub fn rms_db(samples: &[f32]) -> f32 {
    gain_to_db(rms(samples))
}

/// Wartość szczytowa.
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

/// Normalizacja głośności wypowiedzi (strona sterująca — przed kolejką RT).
///
/// Wzmocnienie dąży do `target_db` RMS dla fragmentów z sygnałem (bramka `gate_db`), wolno
/// w górę i szybciej w dół (bez „pompowania”), ograniczone do `max_gain_db`; szczyt po wzmocnieniu
/// nie przekracza `ceiling`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessNormalizer {
    /// Docelowy poziom RMS (dBFS).
    pub target_db: f32,
    /// Maksymalne wzmocnienie (dB).
    pub max_gain_db: f32,
    /// Poniżej tego poziomu fragment nie zmienia wzmocnienia (cisza, pauzy).
    pub gate_db: f32,
    /// Sufit szczytowy po wzmocnieniu (liniowo).
    pub ceiling: f32,
    gain_db: f32,
    primed: bool,
}

impl Default for LoudnessNormalizer {
    fn default() -> Self {
        Self::new(-20.0)
    }
}

impl LoudnessNormalizer {
    /// Nowy normalizator z celem `target_db`.
    pub fn new(target_db: f32) -> Self {
        Self {
            target_db,
            max_gain_db: 12.0,
            gate_db: -50.0,
            ceiling: 0.89,
            gain_db: 0.0,
            primed: false,
        }
    }

    /// Bieżące wzmocnienie (dB).
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    /// Zeruje stan (nowa wypowiedź innej agentki).
    pub fn reset(&mut self) {
        self.gain_db = 0.0;
        self.primed = false;
    }

    /// Normalizuje fragment w miejscu.
    pub fn process(&mut self, samples: &mut [f32]) {
        let level = rms_db(samples);
        if level > self.gate_db {
            let wanted = (self.target_db - level).clamp(-30.0, self.max_gain_db);
            if self.primed {
                // W górę wolno (0,3), w dół szybciej (0,7) — bez pompowania na pauzach.
                let k = if wanted > self.gain_db { 0.3 } else { 0.7 };
                self.gain_db += (wanted - self.gain_db) * k;
            } else {
                self.gain_db = wanted;
                self.primed = true;
            }
        }
        let mut g = db_to_gain(self.gain_db);
        let p = peak(samples) * g;
        if p > self.ceiling {
            g *= self.ceiling / p;
        }
        for s in samples.iter_mut() {
            *s *= g;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::sine;

    #[test]
    fn db_roundtrip_and_levels() {
        assert!((db_to_gain(-6.0206) - 0.5).abs() < 1e-4);
        assert!((gain_to_db(0.1) + 20.0).abs() < 1e-4);
        assert_eq!(gain_to_db(0.0), SILENCE_DB);
        assert_eq!(rms(&[]), 0.0);
        let s = sine(1000.0, 48_000, 0.1, 1.0);
        assert!((rms_db(&s) + 3.01).abs() < 0.05);
        assert!((peak(&s) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn normalizer_converges_to_target_from_both_sides() {
        for amp_db in [-40.0f32, -30.0, -10.0, -3.0] {
            let mut n = LoudnessNormalizer::new(-20.0);
            let amp = db_to_gain(amp_db + 3.01); // sinus: RMS = szczyt − 3 dB
            let mut last = 0.0;
            for _ in 0..20 {
                let mut chunk = sine(440.0, 24_000, 0.1, amp);
                n.process(&mut chunk);
                last = rms_db(&chunk);
            }
            let expected = (-20.0f32).min(amp_db + 12.0);
            assert!(
                (last - expected).abs() < 1.0,
                "{amp_db}: {last} vs {expected}"
            );
        }
    }

    #[test]
    fn normalizer_respects_ceiling_and_gate() {
        let mut n = LoudnessNormalizer::new(-3.0);
        let mut loud = sine(440.0, 24_000, 0.1, 0.9);
        n.process(&mut loud);
        assert!(peak(&loud) <= 0.89 + 1e-4);
        let mut quiet = vec![1e-4f32; 480];
        let before = n.gain_db();
        n.process(&mut quiet);
        assert_eq!(n.gain_db(), before);
        n.reset();
        assert_eq!(n.gain_db(), 0.0);
    }
}
