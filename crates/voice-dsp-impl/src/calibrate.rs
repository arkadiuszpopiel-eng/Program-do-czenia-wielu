//! Autokalibracja opóźnienia pętli: korelacja wzajemna (FFT, `rustfft`) nagrania z sygnałem testowym.

use std::time::Duration;

use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;
use voice_dsp_contract::{Calibration, DspError};

/// Najdłuższe szukane opóźnienie pętli.
pub const MAX_LOOP: Duration = Duration::from_millis(500);
/// Minimalna pewność (znormalizowana korelacja szczytu).
pub const MIN_CONFIDENCE: f32 = 0.2;

/// Szacuje opóźnienie `recorded` względem `played` (obie w `rate`, wyrównane czasem rozpoczęcia).
pub fn estimate_delay(
    played: &[f32],
    recorded: &[f32],
    rate: u32,
) -> Result<Calibration, DspError> {
    if played.is_empty() || recorded.is_empty() {
        return Err(DspError::Calibration("puste nagranie".into()));
    }
    let n = (played.len() + recorded.len()).next_power_of_two();
    let mut planner = FftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(n);
    let inv = planner.plan_fft_inverse(n);
    let to_c = |x: &[f32]| {
        let mut v: Vec<Complex32> = x.iter().map(|&r| Complex32::new(r, 0.0)).collect();
        v.resize(n, Complex32::new(0.0, 0.0));
        v
    };
    let (mut a, mut b) = (to_c(played), to_c(recorded));
    fwd.process(&mut a);
    fwd.process(&mut b);
    let mut r: Vec<Complex32> = a.iter().zip(&b).map(|(x, y)| x.conj() * y).collect();
    inv.process(&mut r);
    let max_lag = ((u64::from(rate) * MAX_LOOP.as_millis() as u64 / 1000) as usize)
        .min(recorded.len().saturating_sub(1));
    let (lag, peak) = (0..=max_lag)
        .map(|k| (k, r[k].re / n as f32))
        .max_by(|x, y| x.1.abs().total_cmp(&y.1.abs()))
        .ok_or_else(|| DspError::Calibration("brak danych".into()))?;
    let e_played: f32 = played.iter().map(|x| x * x).sum();
    let end = (lag + played.len()).min(recorded.len());
    let e_rec: f32 = recorded[lag..end].iter().map(|x| x * x).sum();
    if e_played <= 0.0 || e_rec <= 0.0 {
        return Err(DspError::Calibration("cisza w nagraniu".into()));
    }
    let confidence = (peak.abs() / (e_played * e_rec).sqrt()).clamp(0.0, 1.0);
    if confidence < MIN_CONFIDENCE {
        return Err(DspError::Calibration(format!(
            "sygnał testowy niesłyszalny w mikrofonie (pewność {confidence:.2})"
        )));
    }
    let gain = peak.abs() / e_played;
    Ok(Calibration {
        loop_delay: Duration::from_nanos(lag as u64 * 1_000_000_000 / u64::from(rate)),
        attenuation_db: voice_audio_contract::gain::gain_to_db(gain),
        confidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::synth::white_noise;
    use voice_dsp_contract::calibration_signal;

    fn record(delay: usize, gain: f32, noise: f32) -> (Vec<f32>, Vec<f32>) {
        let played = calibration_signal(16_000);
        let mut rec = white_noise(9, played.len() + 9_000, noise);
        for (i, s) in played.iter().enumerate() {
            rec[i + delay] += gain * s;
            if i + delay + 40 < rec.len() {
                rec[i + delay + 40] += 0.3 * gain * s; // odbicie
            }
        }
        (played, rec)
    }

    #[test]
    fn finds_delay_within_one_ms() {
        for (delay, gain) in [(1_968usize, 0.3f32), (320, 0.05), (6_400, 0.5)] {
            let (p, r) = record(delay, gain, 0.003);
            let c = estimate_delay(&p, &r, 16_000).unwrap();
            let want = Duration::from_micros(delay as u64 * 1_000_000 / 16_000);
            let diff = c.loop_delay.as_micros().abs_diff(want.as_micros());
            assert!(diff <= 1_000, "{delay}: {:?} vs {want:?}", c.loop_delay);
            assert!((c.attenuation_db - voice_audio_contract::gain::gain_to_db(gain)).abs() < 3.0);
            assert!(c.confidence > 0.5);
        }
    }

    #[test]
    fn rejects_silence_and_unrelated_noise() {
        let played = calibration_signal(16_000);
        assert!(estimate_delay(&played, &vec![0.0; 20_000], 16_000).is_err());
        assert!(estimate_delay(&played, &white_noise(3, 20_000, 0.1), 16_000).is_err());
        assert!(estimate_delay(&[], &[0.1], 16_000).is_err());
    }
}
