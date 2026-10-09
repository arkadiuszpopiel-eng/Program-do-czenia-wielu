//! Dwa proste stopnie wokół RNNoise, które robią różnicę na laptopie z głośnym chłodzeniem:
//! filtr górnoprzepustowy (dudnienie wentylatorów i przydźwięk sieci leżą poniżej mowy) oraz
//! bramka szumów — między wypowiedziami to, co RNNoise zostawił, jest ściszane o zadaną liczbę
//! decybeli. Bramka nie wycina do zera (cisza „cyfrowa” brzmi nienaturalnie i myli VAD), otwiera
//! się natychmiast, a zamyka łagodnie po krótkim podtrzymaniu, żeby nie ucinać końcówek słów.

use voice_audio_contract::gain::db_to_gain;

/// Filtr górnoprzepustowy drugiego rzędu (Butterworth, postać bezpośrednia transponowana II).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighPass {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}

impl HighPass {
    /// Filtr o częstotliwości odcięcia `cutoff_hz` przy próbkowaniu `rate`.
    pub fn new(cutoff_hz: f32, rate: u32) -> Self {
        let w = 2.0 * std::f32::consts::PI * cutoff_hz / rate as f32;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / std::f32::consts::SQRT_2;
        let a0 = 1.0 + alpha;
        Self {
            b: [
                (1.0 + cos) / 2.0 / a0,
                -(1.0 + cos) / a0,
                (1.0 + cos) / 2.0 / a0,
            ],
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            z: [0.0; 2],
        }
    }

    /// Filtruje blok w miejscu.
    pub fn process(&mut self, block: &mut [f32]) {
        for sample in block.iter_mut() {
            let x = *sample;
            let y = self.b[0] * x + self.z[0];
            self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
            self.z[1] = self.b[2] * x - self.a[1] * y;
            *sample = y;
        }
    }
}

/// Prawdopodobieństwo mowy, od którego bramka się otwiera.
const OPEN_AT: f32 = 0.5;
/// Prawdopodobieństwo, przy którym otwarta bramka jeszcze się nie zamyka (histereza).
const STAY_OPEN_AT: f32 = 0.3;
/// Podtrzymanie po ostatniej ramce mowy (bloki 10 ms) — końcówki słów i krótkie pauzy.
const HOLD_BLOCKS: u32 = 30;
/// Udział drogi do celu pokonywany w jednym bloku przy zamykaniu (ok. 150 ms do −18 dB).
const RELEASE: f32 = 0.12;

/// Bramka szumów sterowana prawdopodobieństwem mowy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseGate {
    floor: f32,
    gain: f32,
    hold: u32,
    open: bool,
}

impl NoiseGate {
    /// Bramka ściszająca ramki bez mowy o `attenuation_db` (0 = bramka wyłączona).
    pub fn new(attenuation_db: f32) -> Self {
        Self {
            floor: db_to_gain(-attenuation_db.max(0.0)),
            gain: 1.0,
            hold: 0,
            open: true,
        }
    }

    /// Bieżące wzmocnienie bramki (1 = otwarta).
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Przetwarza blok w miejscu; `speech_prob` — prawdopodobieństwo mowy w tym bloku.
    pub fn process(&mut self, block: &mut [f32], speech_prob: f32) {
        let threshold = if self.open { STAY_OPEN_AT } else { OPEN_AT };
        if speech_prob >= threshold {
            self.open = true;
            self.hold = HOLD_BLOCKS;
        } else if self.hold > 0 {
            self.hold -= 1;
        } else {
            self.open = false;
        }
        let from = self.gain;
        // Otwarcie natychmiastowe (początek słowa), zamknięcie łagodne.
        self.gain = if self.open {
            1.0
        } else {
            self.gain + (self.floor - self.gain) * RELEASE
        };
        let n = block.len().max(1) as f32;
        for (i, sample) in block.iter_mut().enumerate() {
            *sample *= from + (self.gain - from) * (i as f32 / n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::gain::rms_db;
    use voice_audio_contract::synth::{sine, white_noise};

    fn filtered_db(freq: f32) -> f32 {
        let mut hp = HighPass::new(100.0, 48_000);
        let mut tone = sine(freq, 48_000, 1.0, 0.5);
        let before = rms_db(&tone);
        hp.process(&mut tone);
        // Pomijamy stan przejściowy filtra.
        rms_db(&tone[24_000..]) - before
    }

    #[test]
    fn high_pass_removes_rumble_and_keeps_speech() {
        assert!(filtered_db(30.0) < -18.0, "30 Hz: {}", filtered_db(30.0));
        assert!(filtered_db(50.0) < -10.0, "50 Hz: {}", filtered_db(50.0));
        assert!(
            filtered_db(300.0).abs() < 0.5,
            "300 Hz: {}",
            filtered_db(300.0)
        );
        assert!(filtered_db(3_000.0).abs() < 0.1);
    }

    #[test]
    fn gate_attenuates_background_and_opens_at_once_for_speech() {
        let mut gate = NoiseGate::new(18.0);
        let noise = white_noise(7, 480, 0.05);
        let level = rms_db(&noise);
        // Sekunda samego tła: po podtrzymaniu bramka schodzi do −18 dB.
        let mut last = 0.0;
        for _ in 0..100 {
            let mut block = noise.clone();
            gate.process(&mut block, 0.02);
            last = rms_db(&block);
        }
        assert!((last - (level - 18.0)).abs() < 1.0, "{last} wobec {level}");
        // Mowa: następny blok kończy się już pełnym poziomem.
        let mut block = noise.clone();
        gate.process(&mut block, 0.9);
        assert!((gate.gain() - 1.0).abs() < f32::EPSILON);
        // Krótka pauza w zdaniu (200 ms) nie zamyka bramki.
        for _ in 0..20 {
            let mut block = noise.clone();
            gate.process(&mut block, 0.1);
            assert!((rms_db(&block) - level).abs() < 0.1);
        }
    }

    #[test]
    fn gate_with_zero_attenuation_is_transparent() {
        let mut gate = NoiseGate::new(0.0);
        let noise = white_noise(3, 480, 0.05);
        for _ in 0..200 {
            let mut block = noise.clone();
            gate.process(&mut block, 0.0);
            assert_eq!(block, noise);
        }
    }
}
