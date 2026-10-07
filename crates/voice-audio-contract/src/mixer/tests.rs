use std::time::Duration;

use proptest::prelude::*;

use super::{MixerConfig, MixerReport, mixer};
use crate::frame::MediaTime;
use crate::gain::db_to_gain;
use crate::types::{AudioError, Ducking, Lane, PlaybackState};

const RATE: u32 = 48_000;

fn render_ms(r: &mut super::MixerRender, ms: u32, t: &mut u64) -> Vec<f32> {
    let n = (RATE / 1000 * ms) as usize;
    let mut out = vec![0.0; n * 2];
    r.render(&mut out, 2, MediaTime::from_samples(*t, RATE));
    *t += n as u64;
    out.chunks_exact(2).map(|f| f[0]).collect()
}

#[test]
fn utterance_plays_reports_and_positions() {
    let (mut c, mut r) = mixer(MixerConfig::new(RATE));
    let mut t = 0;
    c.enqueue(Lane::Voice, 1, &vec![0.5; 4_800]).unwrap();
    c.end(Lane::Voice, 1).unwrap();
    assert_eq!(c.position(1), Some((0, 4_800, PlaybackState::Queued)));
    let out = render_ms(&mut r, 50, &mut t);
    assert!(out.iter().all(|&s| (s - 0.5).abs() < 1e-6));
    assert_eq!(c.position(1), Some((2_400, 4_800, PlaybackState::Playing)));
    let out = render_ms(&mut r, 100, &mut t);
    assert!(out[..2_400].iter().all(|&s| (s - 0.5).abs() < 1e-6));
    assert!(out[2_400..].iter().all(|&s| s == 0.0));
    let reports = c.poll();
    assert_eq!(
        reports,
        vec![
            MixerReport::Started {
                lane: Lane::Voice,
                utterance: 1
            },
            MixerReport::Finished {
                lane: Lane::Voice,
                utterance: 1,
                rendered: 4_800,
                stopped: false
            },
        ]
    );
    assert_eq!(c.position(1), Some((4_800, 4_800, PlaybackState::Finished)));
    assert_eq!(c.stats().underruns, 0);
    assert_eq!(c.stats().rendered, 7_200);
    assert!(c.position(99).is_none());
}

#[test]
fn voice_lane_accepts_one_open_utterance_and_queues_sequential_ones() {
    let (mut c, mut r) = mixer(MixerConfig::new(RATE));
    let mut t = 0;
    c.enqueue(Lane::Voice, 1, &[0.1; 480]).unwrap();
    assert_eq!(
        c.enqueue(Lane::Voice, 2, &[0.2; 480]),
        Err(AudioError::VoiceBusy { playing: 1 })
    );
    c.enqueue(Lane::Effects, 50, &[0.05; 480]).unwrap();
    c.end(Lane::Voice, 1).unwrap();
    c.enqueue(Lane::Voice, 2, &[0.2; 480]).unwrap();
    c.end(Lane::Voice, 2).unwrap();
    let out = render_ms(&mut r, 20, &mut t);
    assert!((out[0] - 0.15).abs() < 1e-6, "głos + earcon");
    assert!((out[480] - 0.2).abs() < 1e-6, "druga wypowiedź bez przerwy");
    let finished: Vec<u64> = c
        .poll()
        .into_iter()
        .filter_map(|r| match r {
            MixerReport::Finished {
                utterance,
                lane: Lane::Voice,
                ..
            } => Some(utterance),
            _ => None,
        })
        .collect();
    assert_eq!(finished, vec![1, 2]);
}

#[test]
fn ducking_reaches_target_within_attack() {
    let (mut c, mut r) = mixer(MixerConfig::new(RATE));
    let mut t = 0;
    c.enqueue(Lane::Voice, 1, &vec![0.5; RATE as usize])
        .unwrap();
    render_ms(&mut r, 10, &mut t);
    c.duck(Ducking::default()).unwrap();
    let out = render_ms(&mut r, 30, &mut t);
    let target = 0.5 * db_to_gain(-15.0);
    // Rampa 20 ms = 960 próbek; później stały poziom −15 dB.
    assert!(out[400] > target && out[400] < 0.5);
    assert!(out[960..].iter().all(|&s| (s - target).abs() < 1e-4));
    assert!((c.duck_gain() - db_to_gain(-15.0)).abs() < 1e-4);
    c.unduck(Duration::from_millis(10)).unwrap();
    let out = render_ms(&mut r, 20, &mut t);
    assert!((out[900] - 0.5).abs() < 1e-4);
    assert!(
        c.duck(Ducking {
            attack: Duration::from_millis(60),
            ..Ducking::default()
        })
        .is_err()
    );
}

#[test]
fn stop_fades_flushes_and_keeps_later_chunks_in_sync() {
    let (mut c, mut r) = mixer(MixerConfig::new(RATE));
    let mut t = 0;
    c.enqueue(Lane::Voice, 1, &vec![0.5; RATE as usize])
        .unwrap();
    render_ms(&mut r, 10, &mut t);
    c.stop(Lane::Voice, Duration::from_millis(5)).unwrap();
    // Spóźnione fragmenty przerwanej wypowiedzi są ignorowane.
    c.enqueue(Lane::Voice, 1, &[0.9; 480]).unwrap();
    c.enqueue(Lane::Voice, 2, &[0.25; 480]).unwrap();
    c.end(Lane::Voice, 2).unwrap();
    let out = render_ms(&mut r, 30, &mut t);
    assert!(out[0] <= 0.5 && out[0] > 0.45, "początek wygaszania");
    assert!(out[239] < 0.01, "koniec wygaszania po 5 ms");
    assert!(
        out[240..720].iter().all(|&s| (s - 0.25).abs() < 1e-6),
        "następna wypowiedź"
    );
    assert!(out[720..].iter().all(|&s| s == 0.0));
    let (rendered, _, state) = c.position(1).unwrap();
    assert_eq!(state, PlaybackState::Stopped);
    assert_eq!(rendered, 480 + 240);
    assert_eq!(c.position(2).unwrap().2, PlaybackState::Finished);
}

#[test]
fn underrun_is_reported_once_per_starvation() {
    let (mut c, mut r) = mixer(MixerConfig::new(RATE));
    let mut t = 0;
    c.enqueue(Lane::Voice, 7, &[0.1; 480]).unwrap();
    render_ms(&mut r, 30, &mut t);
    render_ms(&mut r, 30, &mut t);
    let underruns = c
        .poll()
        .into_iter()
        .filter(|r| matches!(r, MixerReport::Underrun { .. }))
        .count();
    assert_eq!(underruns, 1);
    assert_eq!(c.stats().underruns, 1);
}

#[test]
fn reference_matches_output_with_timestamps() {
    let (mut c, mut r) = mixer(MixerConfig::new(RATE));
    let mut t = 48_000;
    c.enqueue(Lane::Voice, 1, &[0.3; 960]).unwrap();
    let out = render_ms(&mut r, 10, &mut t);
    let _ = render_ms(&mut r, 10, &mut t);
    let refs = c.drain_reference();
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].ts.as_ms(), 1_000);
    assert_eq!(refs[1].ts.as_ms(), 1_010);
    assert_eq!(&refs[0].pcm[..], &out[..]);
    let (mut c2, mut r2) = mixer(MixerConfig {
        reference: false,
        ..MixerConfig::new(RATE)
    });
    render_ms(&mut r2, 10, &mut t);
    assert!(c2.drain_reference().is_empty());
    c2.set_gain(Lane::Effects, -6.0).unwrap();
    c2.enqueue(Lane::Effects, 1, &[0.5; 10]).unwrap();
    let out = render_ms(&mut r2, 1, &mut t);
    assert!((out[0] - 0.5 * db_to_gain(-6.0)).abs() < 1e-4);
    assert_eq!(c2.sample_rate(), RATE);
}

#[test]
fn queue_full_is_reported_without_desync() {
    let cfg = MixerConfig {
        voice_capacity_ms: 100,
        ..MixerConfig::new(RATE)
    };
    let (mut c, mut r) = mixer(cfg);
    let mut t = 0;
    c.enqueue(Lane::Voice, 1, &vec![0.1; 4_000]).unwrap();
    assert_eq!(
        c.enqueue(Lane::Voice, 1, &vec![0.1; 4_000]),
        Err(AudioError::QueueFull)
    );
    c.enqueue(Lane::Voice, 1, &[0.2; 100]).unwrap();
    c.end(Lane::Voice, 1).unwrap();
    let out = render_ms(&mut r, 100, &mut t);
    assert!((out[3_999] - 0.1).abs() < 1e-6 && (out[4_000] - 0.2).abs() < 1e-6);
    assert_eq!(
        c.position(1).unwrap(),
        (4_100, 4_100, PlaybackState::Finished)
    );
    assert!(
        c.enqueue(Lane::Voice, 1, &[0.1; 10]).is_ok(),
        "zamknięta — ignorowane"
    );
    assert!(c.end(Lane::Voice, 404).is_ok());
}

#[derive(Debug, Clone)]
enum Op {
    Enqueue(u64, usize),
    End(u64),
    Stop,
    Render(u32),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (1u64..5, 1usize..2_000).prop_map(|(u, n)| Op::Enqueue(u, n)),
        (1u64..5).prop_map(Op::End),
        Just(Op::Stop),
        (1u32..40).prop_map(Op::Render),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    /// Niezmienniki: wyjście ograniczone, wyrenderowane ≤ zakolejkowane, po zamknięciu i
    /// wyczerpaniu każda nieprzerwana wypowiedź zagrała w całości (brak rozjazdu kolejek).
    #[test]
    fn mixer_invariants(ops in proptest::collection::vec(op(), 1..60)) {
        let (mut c, mut r) = mixer(MixerConfig::new(RATE));
        let mut t = 0;
        for o in ops {
            match o {
                Op::Enqueue(u, n) => { let _ = c.enqueue(Lane::Voice, u, &vec![0.4; n]); }
                Op::End(u) => { let _ = c.end(Lane::Voice, u); }
                Op::Stop => { let _ = c.stop(Lane::Voice, Duration::from_millis(5)); }
                Op::Render(ms) => {
                    let out = render_ms(&mut r, ms, &mut t);
                    prop_assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
                }
            }
        }
        for u in 1..5 { let _ = c.end(Lane::Voice, u); }
        render_ms(&mut r, 2_000, &mut t);
        for u in 1..5 {
            if let Some((rendered, queued, state)) = c.position(u) {
                prop_assert!(rendered <= queued);
                if state == PlaybackState::Finished {
                    prop_assert_eq!(rendered, queued);
                }
                prop_assert!(matches!(state, PlaybackState::Finished | PlaybackState::Stopped));
            }
        }
    }
}
