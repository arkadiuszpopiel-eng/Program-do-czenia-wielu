//! Modyfikacja głosu v0 (PCM): zmiana tempa **WSOLA** (okna Hanna 20 ms, zakładka 50%, szukanie
//! najlepszego dopasowania ±10 ms korelacją) i zmiana wysokości = WSOLA + resampling (resampler
//! sinc z `voice-audio-contract`). Długość wyjścia = `len / tempo`, F0 × `pitch`.

use voice_audio_contract::Resampler;

/// Długość okna (s).
const FRAME_S: f32 = 0.02;
/// Zakres szukania dopasowania (s).
const TOLERANCE_S: f32 = 0.01;

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos())
        .collect()
}

fn similarity(a: &[f32], b: &[f32]) -> f32 {
    let (mut num, mut ea, mut eb) = (0.0f32, 1e-9f32, 1e-9f32);
    for (x, y) in a.iter().zip(b) {
        num += x * y;
        ea += x * x;
        eb += y * y;
    }
    num / (ea * eb).sqrt()
}

/// Zmiana tempa bez zmiany wysokości (WSOLA). `tempo` > 1 = szybciej (krócej).
pub fn time_stretch(x: &[f32], tempo: f32, rate: u32) -> Vec<f32> {
    if (tempo - 1.0).abs() < 1e-3 || x.is_empty() {
        return x.to_vec();
    }
    let tempo = tempo.clamp(0.25, 4.0);
    let n = ((rate as f32 * FRAME_S) as usize / 2 * 2).max(4);
    let hs = n / 2;
    let ha = hs as f32 * tempo;
    let tol = (rate as f32 * TOLERANCE_S) as isize;
    let window = hann(n);
    let out_len = (x.len() as f32 / tempo).round() as usize;
    let mut out = vec![0.0f32; out_len + n];
    let mut wsum = vec![0.0f32; out_len + n];
    let get = |i: isize| {
        if i >= 0 && (i as usize) < x.len() {
            x[i as usize]
        } else {
            0.0
        }
    };
    let mut prev_start: isize = 0;
    let mut k = 0usize;
    while k * hs < out_len {
        let nominal = (k as f32 * ha).round() as isize;
        let start = if k == 0 {
            0
        } else {
            // Naturalna kontynuacja poprzedniego okna; szukamy okna wokół pozycji nominalnej,
            // którego początek najlepiej pasuje do tej kontynuacji.
            let target: Vec<f32> = (0..hs as isize)
                .map(|i| get(prev_start + hs as isize + i))
                .collect();
            let mut best = (nominal, f32::MIN);
            for d in -tol..=tol {
                let cand = nominal + d;
                if cand < 0 {
                    continue;
                }
                let seg: Vec<f32> = (0..hs as isize).map(|i| get(cand + i)).collect();
                let s = similarity(&seg, &target);
                if s > best.1 {
                    best = (cand, s);
                }
            }
            best.0
        };
        let base = k * hs;
        for (i, w) in window.iter().enumerate() {
            out[base + i] += get(start + i as isize) * w;
            wsum[base + i] += w;
        }
        prev_start = start;
        k += 1;
    }
    for (o, w) in out.iter_mut().zip(&wsum) {
        if *w > 1e-3 {
            *o /= w;
        }
    }
    out.truncate(out_len);
    out
}

/// Zmiana wysokości (`pitch` × F0) i tempa (`tempo`): WSOLA do długości `len · pitch / tempo`,
/// potem resampling 1/`pitch` (długość `len / tempo`, wysokość × `pitch`).
pub fn apply_preset(x: &[f32], rate: u32, pitch: f32, tempo: f32) -> Vec<f32> {
    let pitch = pitch.clamp(0.5, 2.0);
    if (pitch - 1.0).abs() < 1e-3 {
        return time_stretch(x, tempo, rate);
    }
    let stretched = time_stretch(x, tempo / pitch, rate);
    Resampler::convert_ratio(rate, rate, f64::from(1.0 / pitch), &stretched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::synth::{SpeechParams, estimate_f0, synthetic_speech};

    const RATE: u32 = 24_000;

    /// Sygnał harmoniczny (F0 + 5 harmonicznych) — „głos” o stałej wysokości.
    fn voiced(f0: f32, secs: f32) -> Vec<f32> {
        let n = (secs * RATE as f32) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                (1..=6)
                    .map(|h| (2.0 * std::f32::consts::PI * f0 * h as f32 * t).sin() / h as f32)
                    .sum::<f32>()
                    * 0.3
            })
            .collect()
    }

    fn f0(x: &[f32]) -> f32 {
        let mid = &x[x.len() / 3..x.len() / 3 + 4_800];
        estimate_f0(mid, RATE, 60.0, 600.0).unwrap()
    }

    #[test]
    fn pitch_changes_f0_within_3_percent_and_keeps_duration() {
        let x = voiced(200.0, 1.5);
        for p in [0.88f32, 0.92, 1.06, 1.12, 1.2] {
            let y = apply_preset(&x, RATE, p, 1.0);
            let got = f0(&y);
            let err_f0 = (got / (200.0 * p) - 1.0).abs();
            let err_len = (y.len() as f32 / x.len() as f32 - 1.0).abs();
            eprintln!(
                "pitch ×{p}: F0 {got:.1} Hz (błąd {:.2}%), długość błąd {:.2}%",
                err_f0 * 100.0,
                err_len * 100.0
            );
            assert!(
                err_f0 < 0.03,
                "pitch {p}: F0 {got} Hz (błąd {:.2}%)",
                err_f0 * 100.0
            );
            assert!(
                err_len < 0.03,
                "pitch {p}: długość ×{}",
                y.len() as f32 / x.len() as f32
            );
        }
    }

    #[test]
    fn tempo_changes_duration_within_3_percent_and_keeps_f0() {
        let x = voiced(180.0, 1.5);
        for t in [0.8f32, 0.92, 1.08, 1.25] {
            let y = time_stretch(&x, t, RATE);
            let want = x.len() as f32 / t;
            eprintln!(
                "tempo ×{t}: długość błąd {:.2}%, F0 {:.1} Hz",
                (y.len() as f32 / want - 1.0).abs() * 100.0,
                f0(&y)
            );
            assert!((y.len() as f32 / want - 1.0).abs() < 0.03, "tempo {t}");
            let got = f0(&y);
            assert!((got / 180.0 - 1.0).abs() < 0.03, "tempo {t}: F0 {got}");
        }
        let both = apply_preset(&x, RATE, 1.1, 1.2);
        assert!((both.len() as f32 / (x.len() as f32 / 1.2) - 1.0).abs() < 0.03);
        assert!((f0(&both) / 198.0 - 1.0).abs() < 0.03);
    }

    #[test]
    fn works_on_synthetic_speech_and_edge_cases() {
        let s = synthetic_speech(
            RATE,
            1.2,
            SpeechParams {
                f0: 210.0,
                ..SpeechParams::default()
            },
        );
        let y = apply_preset(&s, RATE, 1.12, 1.08);
        assert!((y.len() as f32 / (s.len() as f32 / 1.08) - 1.0).abs() < 0.03);
        assert!(y.iter().all(|v| v.is_finite() && v.abs() < 1.5));
        assert_eq!(time_stretch(&[], 1.5, RATE), Vec::<f32>::new());
        assert_eq!(time_stretch(&[0.1, 0.2], 1.0, RATE), vec![0.1, 0.2]);
    }
}
