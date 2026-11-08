//! Deterministyczne sygnały testowe i analiza (testy wszystkich modułów głosu, kalibracja pętli):
//! sinus, szum (xorshift z ziarnem), „mowa syntetyczna”, chirp, estymacja F0, korelacja.

use std::f64::consts::PI;

/// Deterministyczny generator pseudolosowy (xorshift64*), bez zależności zewnętrznych.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// Generator z ziarnem (0 zamieniane na stałą).
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// Następna liczba 64-bitowa.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Jednostajnie w [-1, 1).
    pub fn uniform(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }

    /// Przybliżenie rozkładu normalnego (suma 4 jednostajnych), odchylenie ≈ 1.
    pub fn gaussian(&mut self) -> f32 {
        (self.uniform() + self.uniform() + self.uniform() + self.uniform()) * 0.866
    }
}

/// Sinus `freq` Hz o amplitudzie `amp`, `secs` sekund.
pub fn sine(freq: f32, rate: u32, secs: f32, amp: f32) -> Vec<f32> {
    let n = (secs * rate as f32).round() as usize;
    (0..n)
        .map(|i| {
            (amp as f64 * (2.0 * PI * f64::from(freq) * i as f64 / f64::from(rate)).sin()) as f32
        })
        .collect()
}

/// Szum biały (gaussowski) o poziomie RMS `rms`.
pub fn white_noise(seed: u64, n: usize, rms: f32) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| rng.gaussian() * rms).collect()
}

/// Cisza.
pub fn silence(n: usize) -> Vec<f32> {
    vec![0.0; n]
}

/// Chirp logarytmiczny `f0` → `f1` (sygnał kalibracji pętli: ostra autokorelacja).
pub fn chirp(f0: f32, f1: f32, rate: u32, secs: f32, amp: f32) -> Vec<f32> {
    let n = (secs * rate as f32).round() as usize;
    let (f0, f1, t1) = (f64::from(f0), f64::from(f1), f64::from(secs));
    let k = (f1 / f0).ln() / t1;
    (0..n)
        .map(|i| {
            let t = i as f64 / f64::from(rate);
            let phase = 2.0 * PI * f0 * ((k * t).exp() - 1.0) / k;
            let fade = (i.min(n - i) as f64 / (0.005 * f64::from(rate))).min(1.0);
            (f64::from(amp) * fade * phase.sin()) as f32
        })
        .collect()
}

/// Parametry „mowy syntetycznej”.
#[derive(Debug, Clone, Copy)]
pub struct SpeechParams {
    /// Ton podstawowy (Hz).
    pub f0: f32,
    /// Sylaby na sekundę.
    pub syllable_rate: f32,
    /// Amplituda szczytowa.
    pub amp: f32,
    /// Ziarno (odmiana obwiedni i formantów).
    pub seed: u64,
}

impl Default for SpeechParams {
    fn default() -> Self {
        Self {
            f0: 210.0,
            syllable_rate: 4.0,
            amp: 0.5,
            seed: 7,
        }
    }
}

/// „Mowa syntetyczna”: harmoniczne tonu podstawowego z lekkim vibrato, dwa formanty zmieniające
/// się co sylabę i obwiednia sylab z krótkimi pauzami. Deterministyczna (ziarno).
pub fn synthetic_speech(rate: u32, secs: f32, p: SpeechParams) -> Vec<f32> {
    let n = (secs * rate as f32).round() as usize;
    let fs = f64::from(rate);
    let mut rng = Rng::new(p.seed);
    let syl_len = (fs / f64::from(p.syllable_rate.max(0.5))) as usize;
    let mut out = Vec::with_capacity(n);
    let mut phase = 0.0f64;
    let (mut f1, mut f2) = (700.0f64, 1200.0f64);
    let mut syl_left = 0usize;
    let mut syl_total = 1usize;
    let mut voiced = true;
    for i in 0..n {
        if syl_left == 0 {
            syl_total =
                (syl_len as f64 * (0.7 + 0.6 * f64::from(rng.uniform().abs()))) as usize + 1;
            syl_left = syl_total;
            f1 = 500.0 + 400.0 * f64::from(rng.uniform().abs());
            f2 = 1000.0 + 1200.0 * f64::from(rng.uniform().abs());
            voiced = rng.uniform() > -0.8; // ~10% pauz
        }
        let pos = (syl_total - syl_left) as f64 / syl_total as f64;
        syl_left -= 1;
        let env = if voiced {
            (PI * pos).sin().powf(0.6)
        } else {
            0.0
        };
        let t = i as f64 / fs;
        let f0 = f64::from(p.f0) * (1.0 + 0.02 * (2.0 * PI * 5.0 * t).sin());
        phase += 2.0 * PI * f0 / fs;
        let mut s = 0.0;
        let mut h = 1.0;
        while h * f0 < 3_800.0_f64.min(fs * 0.45) {
            let fh = h * f0;
            let formant = 1.0 / (1.0 + ((fh - f1) / 180.0).powi(2))
                + 0.6 / (1.0 + ((fh - f2) / 250.0).powi(2));
            s += (0.15 + formant) / h * (h * phase).sin();
            h += 1.0;
        }
        out.push(s * env);
    }
    let pk = out.iter().fold(0.0f64, |m, s| m.max(s.abs())).max(1e-9);
    out.iter()
        .map(|s| (s / pk * f64::from(p.amp)) as f32)
        .collect()
}

/// Estymacja tonu podstawowego (znormalizowana autokorelacja + interpolacja paraboliczna).
pub fn estimate_f0(samples: &[f32], rate: u32, fmin: f32, fmax: f32) -> Option<f32> {
    let min_lag = (rate as f32 / fmax).floor().max(2.0) as usize;
    let max_lag = (rate as f32 / fmin).ceil() as usize;
    if samples.len() < max_lag * 2 + 2 {
        return None;
    }
    let corr = |lag: usize| -> f64 {
        let (mut num, mut e1, mut e2) = (0.0f64, 0.0f64, 0.0f64);
        for i in 0..samples.len() - lag {
            let (a, b) = (f64::from(samples[i]), f64::from(samples[i + lag]));
            num += a * b;
            e1 += a * a;
            e2 += b * b;
        }
        if e1 <= 0.0 || e2 <= 0.0 {
            0.0
        } else {
            num / (e1 * e2).sqrt()
        }
    };
    let values: Vec<f64> = (min_lag - 1..=max_lag + 1).map(corr).collect();
    let best_val = values[1..values.len() - 1]
        .iter()
        .copied()
        .fold(f64::MIN, f64::max);
    if best_val < 0.3 {
        return None;
    }
    // Pierwsze maksimum lokalne ≥ 90% globalnego (unika błędów oktawy w dół).
    let idx = (1..values.len() - 1).find(|&i| {
        values[i] >= 0.9 * best_val && values[i] >= values[i - 1] && values[i] >= values[i + 1]
    })?;
    let (a, b, c) = (values[idx - 1], values[idx], values[idx + 1]);
    let denom = a - 2.0 * b + c;
    let shift = if denom.abs() > 1e-12 {
        0.5 * (a - c) / denom
    } else {
        0.0
    };
    let lag = (min_lag - 1 + idx) as f64 + shift;
    Some((f64::from(rate) / lag) as f32)
}

/// Współczynnik korelacji Pearsona dwóch sygnałów (na wspólnej długości).
pub fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let (ma, mb) = (
        a[..n].iter().map(|&x| f64::from(x)).sum::<f64>() / n as f64,
        b[..n].iter().map(|&x| f64::from(x)).sum::<f64>() / n as f64,
    );
    let (mut num, mut ea, mut eb) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x, y) = (f64::from(a[i]) - ma, f64::from(b[i]) - mb);
        num += x * y;
        ea += x * x;
        eb += y * y;
    }
    if ea <= 0.0 || eb <= 0.0 {
        0.0
    } else {
        (num / (ea * eb).sqrt()) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gain::rms;

    #[test]
    fn rng_is_deterministic_and_bounded() {
        let a: Vec<u64> = {
            let mut r = Rng::new(1);
            (0..5).map(|_| r.next_u64()).collect()
        };
        let b: Vec<u64> = {
            let mut r = Rng::new(1);
            (0..5).map(|_| r.next_u64()).collect()
        };
        assert_eq!(a, b);
        let mut r = Rng::new(0);
        assert!((0..10_000).all(|_| (-1.0..1.0).contains(&r.uniform())));
        let n = white_noise(3, 48_000, 0.1);
        assert!((rms(&n) - 0.1).abs() < 0.01);
    }

    #[test]
    fn f0_estimation_on_sine_and_speech() {
        let s = sine(220.0, 16_000, 0.2, 0.5);
        let f = estimate_f0(&s, 16_000, 60.0, 500.0).unwrap();
        assert!((f - 220.0).abs() < 1.0, "{f}");
        let sp = synthetic_speech(24_000, 1.0, SpeechParams::default());
        let voiced: Vec<f32> = sp[2_000..6_000].to_vec();
        let f = estimate_f0(&voiced, 24_000, 80.0, 500.0).unwrap();
        assert!((f - 210.0).abs() / 210.0 < 0.05, "{f}");
        assert!(estimate_f0(&silence(8_000), 16_000, 60.0, 500.0).is_none());
        assert!(estimate_f0(&[0.1; 10], 16_000, 60.0, 500.0).is_none());
    }

    #[test]
    fn chirp_and_correlation() {
        let c = chirp(200.0, 4_000.0, 16_000, 0.5, 0.5);
        assert_eq!(c.len(), 8_000);
        assert!((correlation(&c, &c) - 1.0).abs() < 1e-5);
        let inv: Vec<f32> = c.iter().map(|x| -x).collect();
        assert!((correlation(&c, &inv) + 1.0).abs() < 1e-5);
        assert_eq!(correlation(&[], &c), 0.0);
        let sp = synthetic_speech(
            16_000,
            2.0,
            SpeechParams {
                seed: 9,
                ..Default::default()
            },
        );
        assert!(sp.iter().all(|s| s.abs() <= 0.5 + 1e-6));
        assert!(rms(&sp) > 0.05);
    }
}
