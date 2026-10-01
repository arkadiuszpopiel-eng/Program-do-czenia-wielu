use proptest::prelude::*;

use super::Resampler;
use crate::gain::{gain_to_db, rms};
use crate::synth::{sine, white_noise};

/// SNR (dB) wyniku względem idealnego sinusa w części środkowej (bez brzegów).
fn snr_vs_ideal(out: &[f32], freq: f32, rate: u32, amp: f32) -> f32 {
    let ideal = sine(freq, rate, out.len() as f32 / rate as f32, amp);
    let n = out.len().min(ideal.len());
    let (a, b) = (n / 5, n * 4 / 5);
    let err: Vec<f32> = (a..b).map(|i| out[i] - ideal[i]).collect();
    gain_to_db(rms(&ideal[a..b])) - gain_to_db(rms(&err))
}

#[test]
fn passthrough_is_identity() {
    let x = white_noise(1, 1000, 0.1);
    assert_eq!(Resampler::convert(16_000, 16_000, &x), x);
    let mut r = Resampler::new(48_000, 48_000);
    assert!(r.is_passthrough());
    let mut out = Vec::new();
    r.process(&x, &mut out);
    r.flush(&mut out);
    assert_eq!(out, x);
}

#[test]
fn downsample_48k_to_16k_is_clean_and_aligned() {
    let x = sine(1_000.0, 48_000, 0.5, 0.5);
    let y = Resampler::convert(48_000, 16_000, &x);
    assert_eq!(y.len(), 8_000);
    let snr = snr_vs_ideal(&y, 1_000.0, 16_000, 0.5);
    assert!(snr > 60.0, "SNR {snr} dB");
}

#[test]
fn downsample_rejects_above_nyquist() {
    for f in [9_000.0f32, 12_000.0, 20_000.0] {
        let x = sine(f, 48_000, 0.5, 0.5);
        let y = Resampler::convert(48_000, 16_000, &x);
        let mid = &y[1_000..7_000];
        let att = gain_to_db(rms(mid)) - gain_to_db(rms(&x));
        assert!(att < -60.0, "{f} Hz: {att} dB");
    }
    let r = Resampler::new(48_000, 16_000);
    assert!((r.passband_hz() - 7_360.0).abs() < 1.0);
}

#[test]
fn upsample_and_fractional_ratios() {
    let x = sine(1_000.0, 16_000, 0.5, 0.5);
    let y = Resampler::convert(16_000, 48_000, &x);
    assert_eq!(y.len(), 24_000);
    assert!(snr_vs_ideal(&y, 1_000.0, 48_000, 0.5) > 60.0);
    let x = sine(440.0, 22_050, 0.5, 0.5);
    let y = Resampler::convert(22_050, 24_000, &x);
    assert_eq!(y.len(), 12_000);
    assert!(snr_vs_ideal(&y, 440.0, 24_000, 0.5) > 55.0);
}

#[test]
fn flush_returns_exact_length() {
    let mut r = Resampler::new(24_000, 48_000);
    let mut out = Vec::new();
    r.process(&vec![0.1; 240], &mut out);
    assert!(
        out.len() < 480,
        "wyjście opóźnione o {} próbek",
        r.latency_in()
    );
    r.flush(&mut out);
    assert_eq!(out.len(), 480);
    assert_eq!((r.from_rate(), r.to_rate()), (24_000, 48_000));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    /// Przetwarzanie w dowolnych kawałkach daje ten sam wynik co w całości (ciągłość stanu).
    #[test]
    fn chunking_does_not_change_output(cuts in proptest::collection::vec(1usize..700, 1..12)) {
        let x = white_noise(5, 4_000, 0.2);
        let whole = Resampler::convert(48_000, 16_000, &x);
        let mut r = Resampler::new(48_000, 16_000);
        let mut out = Vec::new();
        let mut pos = 0;
        for c in cuts {
            let end = (pos + c).min(x.len());
            r.process(&x[pos..end], &mut out);
            pos = end;
        }
        r.process(&x[pos..], &mut out);
        r.flush(&mut out);
        prop_assert_eq!(out.len(), whole.len());
        for (a, b) in out.iter().zip(&whole) {
            prop_assert!((a - b).abs() < 1e-5);
        }
    }
}

#[test]
fn short_kernel_and_phase_cache_match_reference() {
    let x: Vec<f32> = sine(440.0, 48_000, 0.2, 0.5);
    let run = |mut r: Resampler| -> Vec<f32> {
        let mut out = Vec::new();
        for c in x.chunks(480) {
            r.process(c, &mut out);
        }
        // Wyjście jest wyrównane z wejściem (out[i] ↔ x[3i]); różni się tylko chwila wydania.
        out
    };
    let full = run(Resampler::new(48_000, 16_000));
    let short = run(Resampler::with_zero_crossings(48_000, 16_000, 2));
    assert!(
        Resampler::with_zero_crossings(48_000, 16_000, 8).latency_in()
            < Resampler::new(48_000, 16_000).latency_in()
    );
    let n = full.len().min(short.len());
    assert!(n > 2_000);
    let err = (100..n)
        .map(|i| (full[i] - short[i]).abs())
        .fold(0.0f32, f32::max);
    assert!(err < 0.05, "krótkie jądro bliskie pełnemu: {err}");
}
