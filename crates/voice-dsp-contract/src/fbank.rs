//! Cechy log-mel w stylu Kaldi (`compute-fbank-feats`, `snip_edges = true`, bez ditheru) — wejście
//! modeli słów wywoławczych (`voice-wake` v1) i embeddingów mówcy ECAPA/WeSpeaker (`voice-speaker`).
//!
//! Kroki na ramkę (25 ms co 10 ms @ 16 kHz): usunięcie składowej stałej → preemfaza 0,97 → okno
//! Poveya → FFT (radix-2, własna — bez zależności) → widmo mocy → trójkątne filtry w skali mel
//! (`1127·ln(1 + f/700)`) → `ln(max(e, floor))`. Próbki wejściowe `f32` [-1, 1] są skalowane
//! do zakresu int16 (`input_scale`), jak oczekują modele trenowane na cechach Kaldi.
//! Deterministyczne i bez alokacji w gorącej ścieżce poza wektorami wyjścia.

use crate::DspError;

/// Okno analizy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FbankWindow {
    /// Okno Poveya (Kaldi): `(0,5 − 0,5·cos(2πn/(N−1)))^0,85`.
    Povey,
    /// Okno Hanna.
    Hann,
    /// Okno Hamminga.
    Hamming,
}

/// Parametry cech.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FbankCfg {
    /// Częstotliwość próbkowania.
    pub sample_rate: u32,
    /// Długość ramki (próbki).
    pub frame_len: usize,
    /// Przesunięcie ramki (próbki).
    pub frame_shift: usize,
    /// Rozmiar FFT (potęga dwójki ≥ `frame_len`).
    pub n_fft: usize,
    /// Liczba filtrów mel.
    pub n_mels: usize,
    /// Dolna granica filtrów (Hz).
    pub low_hz: f32,
    /// Górna granica (Hz); `≤ 0` = Nyquist + wartość (Kaldi).
    pub high_hz: f32,
    /// Współczynnik preemfazy (0 = bez).
    pub preemph: f32,
    /// Usuwanie składowej stałej ramki.
    pub remove_dc: bool,
    /// Okno.
    pub window: FbankWindow,
    /// Skala próbek wejściowych (32768 = zakres int16 jak w Kaldi; 1 = bez skalowania).
    pub input_scale: f32,
    /// Dolne ograniczenie energii przed logarytmem.
    pub log_floor: f32,
}

impl FbankCfg {
    /// Kaldi 16 kHz: 25/10 ms, FFT 512, `n_mels` filtrów 20 Hz – Nyquist, okno Poveya.
    pub fn kaldi(n_mels: usize) -> Self {
        Self {
            sample_rate: 16_000,
            frame_len: 400,
            frame_shift: 160,
            n_fft: 512,
            n_mels,
            low_hz: 20.0,
            high_hz: 0.0,
            preemph: 0.97,
            remove_dc: true,
            window: FbankWindow::Povey,
            input_scale: 32_768.0,
            log_floor: f32::EPSILON,
        }
    }

    /// Górna granica filtrów w Hz.
    pub fn high_hz_abs(&self) -> f32 {
        let nyquist = self.sample_rate as f32 / 2.0;
        if self.high_hz <= 0.0 {
            nyquist + self.high_hz
        } else {
            self.high_hz
        }
    }

    /// Walidacja parametrów.
    pub fn validate(&self) -> Result<(), DspError> {
        let bad = |m: &str| Err(DspError::InvalidConfig(format!("fbank: {m}")));
        if self.sample_rate == 0 || self.frame_len == 0 || self.frame_shift == 0 {
            return bad("częstotliwość, długość i przesunięcie ramki muszą być > 0");
        }
        if !self.n_fft.is_power_of_two() || self.n_fft < self.frame_len {
            return bad("n_fft musi być potęgą dwójki ≥ długości ramki");
        }
        let high = self.high_hz_abs();
        if self.n_mels == 0 || self.n_mels > self.n_fft / 2 {
            return bad("liczba filtrów mel 1…n_fft/2");
        }
        if !(0.0..high).contains(&self.low_hz) || high > self.sample_rate as f32 / 2.0 {
            return bad("zakres filtrów poza 0…Nyquist");
        }
        if !(0.0..1.0).contains(&self.preemph) || self.input_scale <= 0.0 || self.log_floor <= 0.0 {
            return bad("preemfaza 0…1, skala i próg logarytmu > 0");
        }
        Ok(())
    }
}

fn mel(hz: f32) -> f32 {
    1127.0 * (1.0 + hz / 700.0).ln()
}

/// Filtr trójkątny: pierwszy prążek i wagi.
#[derive(Debug, Clone)]
struct MelFilter {
    first: usize,
    weights: Vec<f32>,
}

/// Ekstraktor cech log-mel.
#[derive(Debug, Clone)]
pub struct Fbank {
    cfg: FbankCfg,
    window: Vec<f32>,
    filters: Vec<MelFilter>,
}

impl Fbank {
    /// Nowy ekstraktor (okno i filtry liczone raz).
    pub fn new(cfg: FbankCfg) -> Result<Self, DspError> {
        cfg.validate()?;
        let n = cfg.frame_len;
        let denom = (n.max(2) - 1) as f64;
        let window = (0..n)
            .map(|i| {
                let c = (2.0 * std::f64::consts::PI * i as f64 / denom).cos();
                let w = match cfg.window {
                    FbankWindow::Povey => (0.5 - 0.5 * c).powf(0.85),
                    FbankWindow::Hann => 0.5 - 0.5 * c,
                    FbankWindow::Hamming => 0.54 - 0.46 * c,
                };
                w as f32
            })
            .collect();
        let bins = cfg.n_fft / 2;
        let bin_hz = cfg.sample_rate as f32 / cfg.n_fft as f32;
        let (lo, hi) = (mel(cfg.low_hz), mel(cfg.high_hz_abs()));
        let step = (hi - lo) / (cfg.n_mels + 1) as f32;
        let filters = (0..cfg.n_mels)
            .map(|m| {
                let (left, center, right) = (
                    lo + m as f32 * step,
                    lo + (m + 1) as f32 * step,
                    lo + (m + 2) as f32 * step,
                );
                let mut first = None;
                let mut weights = Vec::new();
                for k in 0..bins {
                    let x = mel(bin_hz * k as f32);
                    let w = if x > left && x <= center {
                        (x - left) / (center - left)
                    } else if x > center && x < right {
                        (right - x) / (right - center)
                    } else {
                        0.0
                    };
                    if w > 0.0 {
                        first.get_or_insert(k);
                        weights.push(w);
                    } else if first.is_some() {
                        break;
                    }
                }
                MelFilter {
                    first: first.unwrap_or(0),
                    weights,
                }
            })
            .collect();
        Ok(Self {
            cfg,
            window,
            filters,
        })
    }

    /// Parametry.
    pub fn cfg(&self) -> &FbankCfg {
        &self.cfg
    }

    /// Liczba pełnych ramek z `samples` próbek (`snip_edges`).
    pub fn num_frames(&self, samples: usize) -> usize {
        if samples < self.cfg.frame_len {
            0
        } else {
            1 + (samples - self.cfg.frame_len) / self.cfg.frame_shift
        }
    }

    /// Cechy jednej ramki (`frame.len() == frame_len`) do `out` (`n_mels`).
    pub fn frame(&self, frame: &[f32], out: &mut [f32]) {
        let n_fft = self.cfg.n_fft;
        let mut re = vec![0.0f32; n_fft];
        let mut im = vec![0.0f32; n_fft];
        let n = frame.len().min(self.cfg.frame_len);
        for (dst, src) in re.iter_mut().zip(&frame[..n]) {
            *dst = src * self.cfg.input_scale;
        }
        if self.cfg.remove_dc && n > 0 {
            let mean = re[..n].iter().sum::<f32>() / n as f32;
            re[..n].iter_mut().for_each(|x| *x -= mean);
        }
        if self.cfg.preemph > 0.0 && n > 0 {
            for i in (1..n).rev() {
                re[i] -= self.cfg.preemph * re[i - 1];
            }
            re[0] -= self.cfg.preemph * re[0];
        }
        for (x, w) in re.iter_mut().zip(&self.window) {
            *x *= w;
        }
        fft(&mut re, &mut im);
        for (o, f) in out.iter_mut().zip(&self.filters) {
            let e: f32 = f
                .weights
                .iter()
                .enumerate()
                .map(|(j, w)| {
                    let k = f.first + j;
                    w * (re[k] * re[k] + im[k] * im[k])
                })
                .sum();
            *o = e.max(self.cfg.log_floor).ln();
        }
    }

    /// Cechy całego sygnału (ramki × `n_mels`).
    pub fn compute(&self, samples: &[f32]) -> Vec<Vec<f32>> {
        (0..self.num_frames(samples.len()))
            .map(|i| {
                let start = i * self.cfg.frame_shift;
                let mut out = vec![0.0; self.cfg.n_mels];
                self.frame(&samples[start..start + self.cfg.frame_len], &mut out);
                out
            })
            .collect()
    }
}

/// Strumieniowy ekstraktor: dokładanie próbek → kolejne ramki cech (te same co [`Fbank::compute`]
/// na sklejonym sygnale).
#[derive(Debug, Clone)]
pub struct FbankStream {
    fbank: Fbank,
    pending: Vec<f32>,
}

impl FbankStream {
    /// Strumień na ekstraktorze.
    pub fn new(fbank: Fbank) -> Self {
        Self {
            fbank,
            pending: Vec::new(),
        }
    }

    /// Ekstraktor.
    pub fn fbank(&self) -> &Fbank {
        &self.fbank
    }

    /// Dokłada próbki; zwraca nowe ramki cech.
    pub fn push(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        self.pending.extend_from_slice(samples);
        let frames = self.fbank.compute(&self.pending);
        let consumed = frames.len() * self.fbank.cfg.frame_shift;
        self.pending.drain(..consumed.min(self.pending.len()));
        frames
    }

    /// Reset (porzuca niepełną ramkę).
    pub fn reset(&mut self) {
        self.pending.clear();
    }
}

/// Normalizacja średniej po czasie (CMN) — jak w wejściu modeli mówcy WeSpeaker/3D-Speaker.
pub fn cmn(feats: &mut [Vec<f32>]) {
    let Some(dim) = feats.first().map(Vec::len) else {
        return;
    };
    let n = feats.len() as f32;
    let mut mean = vec![0.0f32; dim];
    for f in feats.iter() {
        for (m, x) in mean.iter_mut().zip(f) {
            *m += x / n;
        }
    }
    for f in feats.iter_mut() {
        for (x, m) in f.iter_mut().zip(&mean) {
            *x -= m;
        }
    }
}

/// FFT radix-2 w miejscu (długość = potęga dwójki).
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len().min(im.len());
    if n < 2 || !n.is_power_of_two() {
        return;
    }
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f64).sin_cos();
                let (wr, wi) = (c as f32, s as f32);
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
#[path = "fbank_tests.rs"]
mod tests;
