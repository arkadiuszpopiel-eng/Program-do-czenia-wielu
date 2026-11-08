//! Testy atrapy DSP: kontrakt współdzielony + skrypt pewności AEC / mowy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use voice_audio_contract::synth::{sine, white_noise};
use voice_audio_contract::{Frame, MediaTime};
use voice_dsp_contract::{Calibration, Dsp, contract_tests};
use voice_dsp_fake::{FakeDsp, ScriptSegment};

#[test]
fn contract_suite() {
    contract_tests::run_all(FakeDsp::new);
}

#[test]
fn script_and_reference_drive_outputs() {
    let mut dsp = FakeDsp::new();
    dsp.script(ScriptSegment {
        start: MediaTime::from_ms(100),
        end: MediaTime::from_ms(200),
        aec_confidence: Some(0.2),
        speech_prob: Some(0.95),
    });
    dsp.set_reference_confidence(0.7);
    dsp.push_reference(&Frame::mono(
        sine(300.0, 48_000, 0.5, 0.3),
        48_000,
        MediaTime::from_ms(250),
    ));
    let noise = white_noise(1, 48_000, 0.001);
    let mut out = Vec::new();
    for (i, c) in noise.chunks(480).enumerate() {
        let f = Frame::mono(c.to_vec(), 48_000, MediaTime::from_ms(10 * i as u64));
        out.extend(dsp.process(&f).unwrap());
    }
    let at = |ms: u64| out.iter().find(|p| p.frame.ts.as_ms() == ms).unwrap();
    assert_eq!(at(150).aec_confidence, 0.2);
    assert!(at(150).speech_likely);
    assert!(!at(50).speech_likely);
    assert_eq!(at(50).aec_confidence, 1.0);
    assert!(at(300).reference_active);
    assert_eq!(at(300).aec_confidence, 0.7);
    assert!(!at(800).reference_active);
    assert!(dsp.calibrate(&[0.1], &[0.1], 16_000).is_err());
    let c = Calibration {
        loop_delay: Duration::from_millis(42),
        attenuation_db: -20.0,
        confidence: 0.9,
    };
    dsp.set_calibration(c);
    assert_eq!(dsp.calibrate(&[0.1], &[0.1], 16_000).unwrap(), c);
    assert_eq!(dsp.take_events().len(), 1);
    assert_eq!(dsp.stats().calibrated_loop, Some(Duration::from_millis(42)));
    dsp.reset();
    assert!(dsp.calibrate(&[], &[0.1], 16_000).is_err());
}
