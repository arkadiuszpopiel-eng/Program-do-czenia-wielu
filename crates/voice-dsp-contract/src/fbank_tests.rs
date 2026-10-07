use super::*;
use voice_audio_contract::synth::{sine, white_noise};

fn naive_dft(x: &[f32]) -> Vec<(f32, f32)> {
    let n = x.len();
    (0..n)
        .map(|k| {
            let (mut r, mut i) = (0.0f64, 0.0f64);
            for (t, v) in x.iter().enumerate() {
                let a = -2.0 * std::f64::consts::PI * (k * t) as f64 / n as f64;
                r += f64::from(*v) * a.cos();
                i += f64::from(*v) * a.sin();
            }
            (r as f32, i as f32)
        })
        .collect()
}

#[test]
fn fft_matches_naive_dft() {
    let x = white_noise(3, 64, 0.3);
    let mut re = x.clone();
    let mut im = vec![0.0; 64];
    fft(&mut re, &mut im);
    for (k, (r, i)) in naive_dft(&x).into_iter().enumerate() {
        assert!((re[k] - r).abs() < 1e-3, "re[{k}] {} vs {r}", re[k]);
        assert!((im[k] - i).abs() < 1e-3, "im[{k}] {} vs {i}", im[k]);
    }
    let mut odd_re = vec![1.0; 6];
    let mut odd_im = vec![0.0; 6];
    fft(&mut odd_re, &mut odd_im);
    assert_eq!(
        odd_re,
        vec![1.0; 6],
        "długość nie-potęga dwójki — bez zmian"
    );
}

#[test]
fn config_validation() {
    assert!(FbankCfg::kaldi(80).validate().is_ok());
    assert!(FbankCfg::kaldi(40).validate().is_ok());
    for bad in [
        FbankCfg {
            n_fft: 300,
            ..FbankCfg::kaldi(80)
        },
        FbankCfg {
            n_mels: 0,
            ..FbankCfg::kaldi(80)
        },
        FbankCfg {
            low_hz: 9_000.0,
            ..FbankCfg::kaldi(80)
        },
        FbankCfg {
            preemph: 1.5,
            ..FbankCfg::kaldi(80)
        },
        FbankCfg {
            frame_shift: 0,
            ..FbankCfg::kaldi(80)
        },
    ] {
        assert!(Fbank::new(bad).is_err(), "{bad:?}");
    }
    assert_eq!(FbankCfg::kaldi(80).high_hz_abs(), 8_000.0);
    let neg = FbankCfg {
        high_hz: -400.0,
        ..FbankCfg::kaldi(80)
    };
    assert_eq!(neg.high_hz_abs(), 7_600.0);
}

#[test]
fn tone_peaks_in_matching_band_and_frames_count() {
    let fb = Fbank::new(FbankCfg::kaldi(40)).unwrap();
    assert_eq!(fb.num_frames(399), 0);
    assert_eq!(fb.num_frames(400), 1);
    assert_eq!(fb.num_frames(16_000), 98);
    let low = fb.compute(&sine(300.0, 16_000, 0.5, 0.3));
    let high = fb.compute(&sine(4_000.0, 16_000, 0.5, 0.3));
    let argmax = |f: &Vec<f32>| {
        f.iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap()
    };
    let (bl, bh) = (argmax(&low[10]), argmax(&high[10]));
    assert!(bl < 10 && bh > 25, "pasma {bl} / {bh}");
    assert!(low.iter().flatten().all(|x| x.is_finite()));
    let silent = fb.compute(&[0.0; 800]);
    assert!(
        silent
            .iter()
            .flatten()
            .all(|x| (*x - f32::EPSILON.ln()).abs() < 1e-3)
    );
}

#[test]
fn stream_equals_batch() {
    let x = white_noise(9, 5_000, 0.2);
    let fb = Fbank::new(FbankCfg::kaldi(80)).unwrap();
    let batch = fb.compute(&x);
    let mut s = FbankStream::new(fb);
    let mut streamed = Vec::new();
    for chunk in x.chunks(137) {
        streamed.extend(s.push(chunk));
    }
    assert_eq!(batch.len(), streamed.len());
    for (a, b) in batch.iter().zip(&streamed) {
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() < 1e-4);
        }
    }
    s.reset();
    assert!(s.push(&x[..100]).is_empty());
    assert_eq!(s.fbank().cfg().n_mels, 80);
}

#[test]
fn cmn_zeroes_mean() {
    let mut f = vec![vec![1.0, 5.0], vec![3.0, 7.0]];
    cmn(&mut f);
    assert_eq!(f, vec![vec![-1.0, -1.0], vec![1.0, 1.0]]);
    let mut empty: Vec<Vec<f32>> = Vec::new();
    cmn(&mut empty);
    for w in [FbankWindow::Hann, FbankWindow::Hamming] {
        let fb = Fbank::new(FbankCfg {
            window: w,
            ..FbankCfg::kaldi(23)
        })
        .unwrap();
        assert_eq!(fb.compute(&sine(1_000.0, 16_000, 0.1, 0.2))[0].len(), 23);
    }
}
