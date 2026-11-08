//! Oś czasu mikrofonu z syntetycznymi „wypowiedziami” użytkownika i statystyka percentyli.

#![allow(dead_code)]

use voice_audio_contract::synth::{SpeechParams, synthetic_speech, white_noise};

/// Częstotliwość osi czasu mikrofonu (jak urządzenie atrapy — bez resamplingu).
pub const RATE: u32 = 48_000;

/// Oś czasu mikrofonu (48 kHz): szum tła + wypowiedzi w zadanych chwilach.
pub struct Timeline {
    pub samples: Vec<f32>,
    /// (start_ms, koniec_ms) wypowiedzi.
    pub utterances: Vec<(u64, u64)>,
}

impl Timeline {
    pub fn new(total_ms: u64, seed: u64) -> Self {
        let n = (total_ms as usize) * (RATE as usize / 1000);
        Self {
            samples: white_noise(seed, n, 0.000_5),
            utterances: Vec::new(),
        }
    }

    /// Syntetyczna „wypowiedź” użytkownika bez pauz (ciągła mowa: harmoniczne 120 Hz, obwiednia
    /// sylab 4 Hz) — niższy głos niż agentki, wyraźnie ponad echem.
    pub fn speech(&mut self, at_ms: u64, ms: u64, seed: u64) {
        let s = voice(ms, seed);
        let start = at_ms as usize * (RATE as usize / 1000);
        for (i, v) in s.iter().enumerate() {
            if let Some(x) = self.samples.get_mut(start + i) {
                *x += v;
            }
        }
        self.utterances.push((at_ms, at_ms + ms));
    }
}

/// Ciągła „mowa” użytkownika: suma harmonicznych z vibrato i obwiednią sylab (bez pauz).
pub fn voice(ms: u64, seed: u64) -> Vec<f32> {
    let n = ms as usize * (RATE as usize / 1000);
    let fs = f64::from(RATE);
    let f0 = 115.0 + (seed % 11) as f64;
    let fade = (0.02 * fs) as usize;
    let mut phase = 0.0f64;
    (0..n)
        .map(|i| {
            let t = i as f64 / fs;
            let f = f0 * (1.0 + 0.03 * (2.0 * std::f64::consts::PI * 5.0 * t).sin());
            phase += 2.0 * std::f64::consts::PI * f / fs;
            let mut v = 0.0;
            for h in 1..=6 {
                v += (h as f64 * phase).sin() / h as f64;
            }
            let syl = 0.6 + 0.4 * (2.0 * std::f64::consts::PI * 4.0 * t + seed as f64).sin();
            let edge = (i.min(n - 1 - i) as f64 / fade as f64).min(1.0);
            (0.32 * v * syl * edge) as f32
        })
        .collect()
}

/// Syntetyczna „wypowiedź” z pauzami jak w mowie naturalnej (`synth::synthetic_speech`).
pub fn natural(ms: u64, seed: u64) -> Vec<f32> {
    synthetic_speech(
        RATE,
        ms as f32 / 1000.0,
        SpeechParams {
            f0: 120.0,
            syllable_rate: 4.0,
            amp: 0.7,
            seed,
        },
    )
}

/// Percentyl (najbliższa ranga) z posortowanej kopii.
pub fn percentile(values: &[u64], p: f64) -> u64 {
    let mut v = values.to_vec();
    v.sort_unstable();
    if v.is_empty() {
        return 0;
    }
    let rank = ((p / 100.0) * v.len() as f64).ceil() as usize;
    v[rank.clamp(1, v.len()) - 1]
}
