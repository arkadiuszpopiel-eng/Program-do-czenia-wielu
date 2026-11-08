//! AEC na syntetycznym echu: referencja = własny strumień TTS, echo = opóźnione (40 ms, nieznane
//! DSP) i przefiltrowane (pokój) wyjście + szum. Kryteria (zadanie F2): ERLE ≥ 15 dB po zbieżności,
//! mowa bliska zachowana. Wyniki wypisywane (`--nocapture`) do raportu.
//!
//! „Zachowana” mierzymy niezależnie od fazy: filtr górnoprzepustowy AEC3 (domyślny w WebRTC)
//! przesuwa fazę niskich harmonicznych, więc korelacja próbek spada do ~0,72 przy niezmienionym
//! widmie (bez HPF: 0,99). Liczy się poziom (±3 dB) i obwiednia (RMS ramek 10 ms) — to widzą VAD i STT.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;
use voice_audio_contract::gain::rms;
use voice_audio_contract::synth::{SpeechParams, correlation, synthetic_speech, white_noise};
use voice_audio_contract::{Frame, MediaTime, Resampler};
use voice_audio_fake::EchoPath;
use voice_dsp_contract::{Dsp, DspCfg};
use voice_dsp_impl::DspPipeline;

const FS: u32 = 48_000;
const OUT: usize = 16_000;

fn convolve(x: &[f32], h: &[f32]) -> Vec<f32> {
    let n = (x.len() + h.len()).next_power_of_two();
    let mut planner = FftPlanner::<f32>::new();
    let (fwd, inv) = (planner.plan_fft_forward(n), planner.plan_fft_inverse(n));
    let pad = |v: &[f32]| {
        let mut c: Vec<Complex32> = v.iter().map(|&r| Complex32::new(r, 0.0)).collect();
        c.resize(n, Complex32::new(0.0, 0.0));
        c
    };
    let (mut a, mut b) = (pad(x), pad(h));
    fwd.process(&mut a);
    fwd.process(&mut b);
    let mut y: Vec<Complex32> = a.iter().zip(&b).map(|(p, q)| p * q).collect();
    inv.process(&mut y);
    y[..x.len()].iter().map(|c| c.re / n as f32).collect()
}

fn db(p: f32) -> f32 {
    10.0 * p.max(1e-20).log10()
}

/// Obwiednia: RMS ramek 10 ms (16 kHz).
fn envelope(x: &[f32]) -> Vec<f32> {
    x.chunks_exact(160).map(rms).collect()
}

/// Najlepsza korelacja obwiedni przy przesunięciu 0–30 ms (opóźnienie AEC ≈ 9 ms).
fn best_env_corr(a: &[f32], b: &[f32]) -> f32 {
    (0..=480)
        .step_by(16)
        .map(|l| correlation(&envelope(&a[l..l + b.len()]), &envelope(b)))
        .fold(f32::MIN, f32::max)
}

struct Scenario {
    far: Vec<f32>,
    near: Vec<f32>,
    mic: Vec<f32>,
}

fn scenario(secs: f32) -> Scenario {
    let n = (secs * FS as f32) as usize;
    let s = |t: f32| ((t * FS as f32) as usize).min(n);
    let mut far = synthetic_speech(
        FS,
        secs,
        SpeechParams {
            f0: 170.0,
            amp: 0.5,
            seed: 11,
            ..Default::default()
        },
    );
    let mut near = synthetic_speech(
        FS,
        secs,
        SpeechParams {
            f0: 250.0,
            amp: 0.3,
            seed: 23,
            syllable_rate: 3.5,
        },
    );
    far[s(6.0)..s(8.0)].fill(0.0); // 6–8 s: tylko mowa bliska
    near[..s(6.0)].fill(0.0); // 0–6 s: tylko echo
    let path = EchoPath::room(Duration::from_millis(40), 0.5, 25, FS, 5);
    let mut h = vec![0.0f32; path.delay_samples(FS)];
    h.extend_from_slice(&path.taps);
    let echo = convolve(&far, &h);
    let noise = white_noise(77, n, 0.0005);
    let mic = (0..n).map(|i| echo[i] + near[i] + noise[i]).collect();
    Scenario { far, near, mic }
}

fn run(dsp: &mut DspPipeline, sc: &Scenario) -> Vec<f32> {
    let frames = sc.mic.len() / 480;
    let frame = |v: &[f32], i: usize| {
        Frame::mono(
            v[i * 480..(i + 1) * 480].to_vec(),
            FS,
            MediaTime::from_ms(10 * i as u64),
        )
    };
    let mut out = Vec::with_capacity(sc.mic.len() / 3);
    for i in 0..frames {
        // Referencja wyprzedza mikrofon (render → bufor urządzenia), jak w `drain_reference`.
        if i == 0 {
            for k in 0..3 {
                dsp.push_reference(&frame(&sc.far, k));
            }
        }
        if i + 3 < frames {
            dsp.push_reference(&frame(&sc.far, i + 3));
        }
        for p in dsp.process(&frame(&sc.mic, i)).unwrap() {
            assert_eq!(p.frame.ts.as_ms() as usize * 16, out.len());
            out.extend_from_slice(&p.frame.pcm);
        }
    }
    out
}

#[test]
fn erle_at_least_15_db_and_near_speech_preserved() {
    let sc = scenario(11.0);
    let mut dsp = DspPipeline::new(DspCfg::aec_only()).unwrap();
    let out = run(&mut dsp, &sc);
    let s48 = |t: f32| (t * FS as f32) as usize;
    let s16 = |t: f32| (t * OUT as f32) as usize;
    // ERLE po zbieżności (3–6 s, tylko echo).
    let erle =
        db(rms(&sc.mic[s48(3.0)..s48(6.0)]).powi(2)) - db(rms(&out[s16(3.0)..s16(6.0)]).powi(2));
    // Mowa bliska bez echa (6,3–8 s): poziom i kształt.
    let near16 = Resampler::convert(FS, OUT as u32, &sc.near);
    let (a, b) = (s16(6.3), s16(8.0));
    let level_diff = db(rms(&out[a..b]).powi(2)) - db(rms(&near16[a..b]).powi(2));
    let corr_near = best_env_corr(&out[a..b + 480], &near16[a..b]);
    // Podwójna rozmowa (8,5–11 s).
    let (c, d) = (s16(8.5), s16(10.9));
    let corr_dt = best_env_corr(&out[c..d + 480], &near16[c..d]);
    let echo16 = Resampler::convert(FS, OUT as u32, &sc.mic);
    let corr_dt_raw = best_env_corr(&echo16[c..d + 480], &near16[c..d]);
    eprintln!(
        "AEC: ERLE {erle:.1} dB; mowa bliska: Δpoziom {level_diff:+.1} dB, korelacja obwiedni {corr_near:.3}; \
         podwójna rozmowa: korelacja obwiedni {corr_dt:.3} (bez AEC {corr_dt_raw:.3}); ERLE wg DSP {:.1} dB",
        dsp.stats().erle_db
    );
    assert!(erle >= 15.0, "ERLE {erle} dB");
    assert!(
        level_diff.abs() < 3.0,
        "poziom mowy bliskiej zmieniony o {level_diff} dB"
    );
    assert!(
        corr_near > 0.95,
        "korelacja obwiedni mowy bliskiej {corr_near}"
    );
    assert!(
        corr_dt > corr_dt_raw,
        "AEC nie poprawił podwójnej rozmowy: {corr_dt} vs {corr_dt_raw}"
    );
    assert!(sc.far.len() == sc.mic.len());
}

#[test]
fn confidence_tracks_echo_cancellation() {
    let sc = scenario(6.0);
    let mut dsp = DspPipeline::new(DspCfg::aec_only()).unwrap();
    let mut conf = Vec::new();
    let frames = sc.mic.len() / 480;
    for i in 0..frames {
        let f = |v: &[f32], k: usize| {
            Frame::mono(
                v[k * 480..(k + 1) * 480].to_vec(),
                FS,
                MediaTime::from_ms(10 * k as u64),
            )
        };
        if i == 0 {
            (0..3).for_each(|k| dsp.push_reference(&f(&sc.far, k)));
        }
        if i + 3 < frames {
            dsp.push_reference(&f(&sc.far, i + 3));
        }
        for p in dsp.process(&f(&sc.mic, i)).unwrap() {
            if p.reference_active {
                conf.push(p.aec_confidence);
            }
        }
    }
    let late: f32 = conf[conf.len() * 3 / 4..].iter().sum::<f32>() / (conf.len() / 4) as f32;
    eprintln!("pewność AEC po zbieżności: {late:.2}");
    assert!(late > 0.6, "{late}");
    assert!(
        dsp.take_events()
            .iter()
            .all(|e| e.name() != voice_dsp_contract::EVENT_ECHO_HIGH)
    );
}
