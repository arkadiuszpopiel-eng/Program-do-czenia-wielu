//! Cechy dźwięku dla Smart Turn: log-mel jak w Whisperze (okno Hanna 400, krok 160, 80 pasm
//! Slaneya, 8 s → tablica 80 × 800). Kolejność działań jak we wzorcowym środowisku wydawcy modelu
//! (`docs/plan/19-specyfikacja-glosu-delty.md` §5) — model musi dostać dokładnie to, na czym był
//! uczony, dlatego zgodność z wzorcem sprawdza test.

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

/// Częstotliwość próbkowania wejścia.
pub const SAMPLE_RATE: u32 = 16_000;
/// Długość okna modelu w próbkach (8 s).
pub const N_SAMPLES: usize = 128_000;
/// Długość okna FFT.
pub const N_FFT: usize = 400;
/// Krok między ramkami.
pub const HOP: usize = 160;
/// Liczba pasm mel.
pub const N_MELS: usize = 80;
/// Liczba ramek (po odrzuceniu ostatniej).
pub const N_FRAMES: usize = N_SAMPLES / HOP;

const N_FREQ: usize = N_FFT / 2 + 1;
const MEL_FLOOR: f64 = 1e-10;
const NORM_EPS: f64 = 1e-7;
const F_SP: f64 = 200.0 / 3.0;
const MIN_LOG_HZ: f64 = 1000.0;

fn log_step() -> f64 {
    6.4f64.ln() / 27.0
}

fn hz_to_mel(freq: f64) -> f64 {
    if freq >= MIN_LOG_HZ {
        MIN_LOG_HZ / F_SP + (freq / MIN_LOG_HZ).ln() / log_step()
    } else {
        freq / F_SP
    }
}

fn mel_to_hz(mel: f64) -> f64 {
    let min_log_mel = MIN_LOG_HZ / F_SP;
    if mel >= min_log_mel {
        MIN_LOG_HZ * (log_step() * (mel - min_log_mel)).exp()
    } else {
        F_SP * mel
    }
}

/// Filtry trójkątne Slaneya: `N_MELS` wierszy po `N_FREQ` wag.
fn filterbank() -> Vec<[f64; N_FREQ]> {
    let nyquist = f64::from(SAMPLE_RATE) / 2.0;
    let (lo, hi) = (hz_to_mel(0.0), hz_to_mel(nyquist));
    let hz: Vec<f64> = (0..N_MELS + 2)
        .map(|i| mel_to_hz(lo + (hi - lo) * i as f64 / (N_MELS + 1) as f64))
        .collect();
    (0..N_MELS)
        .map(|m| {
            let mut row = [0.0; N_FREQ];
            let norm = 2.0 / (hz[m + 2] - hz[m]);
            for (k, w) in row.iter_mut().enumerate() {
                let f = nyquist * k as f64 / (N_FREQ - 1) as f64;
                let lower = (f - hz[m]) / (hz[m + 1] - hz[m]);
                let upper = (hz[m + 2] - f) / (hz[m + 2] - hz[m + 1]);
                *w = lower.min(upper).max(0.0) * norm;
            }
            row
        })
        .collect()
}

/// Ostatnie 8 s nagrania; krótsze dopełnione zerami **z lewej**.
pub fn fit_to_window(audio: &[f32]) -> Vec<f32> {
    if audio.len() >= N_SAMPLES {
        return audio[audio.len() - N_SAMPLES..].to_vec();
    }
    let mut out = vec![0.0; N_SAMPLES - audio.len()];
    out.extend_from_slice(audio);
    out
}

/// Liczy cechy; tablice pomocnicze (okno, filtry, plan FFT) powstają raz.
pub struct MelExtractor {
    fft: Arc<dyn Fft<f64>>,
    window: Vec<f64>,
    filters: Vec<[f64; N_FREQ]>,
}

impl std::fmt::Debug for MelExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MelExtractor").finish_non_exhaustive()
    }
}

impl Default for MelExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl MelExtractor {
    /// Nowy ekstraktor.
    pub fn new() -> Self {
        let window = (0..N_FFT)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / N_FFT as f64).cos())
            .collect();
        Self {
            fft: FftPlanner::new().plan_fft_forward(N_FFT),
            window,
            filters: filterbank(),
        }
    }

    /// Cechy `N_MELS × N_FRAMES` wierszami (pasmo po paśmie) dla nagrania 16 kHz mono.
    pub fn log_mel(&self, audio: &[f32]) -> Vec<f32> {
        let wav = fit_to_window(audio);
        let n = wav.len() as f64;
        let mean = wav.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
        let var = wav
            .iter()
            .map(|&v| (f64::from(v) - mean).powi(2))
            .sum::<f64>()
            / n;
        let scale = 1.0 / (var + NORM_EPS).sqrt();
        let norm: Vec<f64> = wav.iter().map(|&v| (f64::from(v) - mean) * scale).collect();
        // Dopełnienie odbiciem (bez powtórzenia skrajnej próbki), po pół okna z każdej strony.
        let pad = N_FFT / 2;
        let sample = |i: usize| -> f64 {
            if i < pad {
                norm[pad - i]
            } else if i < pad + N_SAMPLES {
                norm[i - pad]
            } else {
                norm[2 * N_SAMPLES + pad - 2 - i]
            }
        };
        let mut out = vec![0.0f32; N_MELS * N_FRAMES];
        let mut buf = vec![Complex::new(0.0, 0.0); N_FFT];
        let mut power = [0.0f64; N_FREQ];
        let mut max = f64::MIN;
        for frame in 0..N_FRAMES {
            for (i, slot) in buf.iter_mut().enumerate() {
                *slot = Complex::new(sample(frame * HOP + i) * self.window[i], 0.0);
            }
            self.fft.process(&mut buf);
            for (p, c) in power.iter_mut().zip(&buf) {
                *p = c.norm_sqr();
            }
            for (m, filter) in self.filters.iter().enumerate() {
                let energy: f64 = filter.iter().zip(&power).map(|(w, p)| w * p).sum();
                let log = energy.max(MEL_FLOOR).log10();
                max = max.max(log);
                out[m * N_FRAMES + frame] = log as f32;
            }
        }
        let floor = (max - 8.0) as f32;
        for v in &mut out {
            *v = (v.max(floor) + 4.0) / 4.0;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_keeps_the_tail_and_pads_on_the_left() {
        let short = fit_to_window(&[1.0, 2.0]);
        assert_eq!(short.len(), N_SAMPLES);
        assert_eq!(&short[N_SAMPLES - 2..], &[1.0, 2.0]);
        assert!(short[..N_SAMPLES - 2].iter().all(|&v| v == 0.0));
        let long: Vec<f32> = (0..N_SAMPLES + 5).map(|i| i as f32).collect();
        let cut = fit_to_window(&long);
        assert_eq!(cut[0], 5.0);
        assert_eq!(cut.len(), N_SAMPLES);
    }

    #[test]
    fn filters_are_triangles_rising_with_frequency() {
        let fb = filterbank();
        assert_eq!(fb.len(), N_MELS);
        for row in &fb {
            assert!(row.iter().all(|&w| w >= 0.0));
            assert!(row.iter().any(|&w| w > 0.0));
        }
        let peak = |row: &[f64; N_FREQ]| {
            row.iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(i, _)| i)
                .unwrap()
        };
        assert!(peak(&fb[0]) < peak(&fb[40]) && peak(&fb[40]) < peak(&fb[79]));
    }

    #[test]
    fn features_have_the_model_shape_and_range() {
        let tone: Vec<f32> = (0..32_000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0).sin() * 0.3)
            .collect();
        let feats = MelExtractor::new().log_mel(&tone);
        assert_eq!(feats.len(), N_MELS * N_FRAMES);
        let max = feats.iter().copied().fold(f32::MIN, f32::max);
        let min = feats.iter().copied().fold(f32::MAX, f32::min);
        assert!(feats.iter().all(|v| v.is_finite()));
        assert!((max - min - 2.0).abs() < 1e-4, "zakres {min}..{max}");
    }
}
