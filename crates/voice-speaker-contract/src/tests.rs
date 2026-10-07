use risk_classifier_contract::{CommandOrigin, RiskLevel};

use crate::eer::{eer, rates_at, report, threshold_for_far};
use crate::*;

/// Deterministyczne „gaussowskie” wyniki (Box–Muller na LCG).
fn scores(seed: u64, n: usize, mean: f32, sd: f32) -> Vec<f32> {
    let mut s = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let mut next = || {
        s = s
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((s >> 11) as f64 / (1u64 << 53) as f64).max(1e-12)
    };
    (0..n)
        .map(|_| {
            let (u, v) = (next(), next());
            let z = (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos();
            mean + sd * z as f32
        })
        .collect()
}

#[test]
fn eer_on_synthetic_scores() {
    let genuine = scores(1, 2_000, 0.75, 0.08);
    let impostor = scores(2, 6_000, 0.15, 0.1);
    let (e, t) = eer(&genuine, &impostor).unwrap();
    assert!(e < 0.001, "EER {e}");
    assert!(t > 0.35 && t < 0.55, "próg {t}");
    let overlap_g = scores(3, 2_000, 0.6, 0.1);
    let overlap_i = scores(4, 2_000, 0.4, 0.1);
    let (e2, _) = eer(&overlap_g, &overlap_i).unwrap();
    // d' = 2 → EER = Φ(−1) ≈ 15,9%.
    assert!((e2 - 0.159).abs() < 0.02, "EER {e2}");
    let strict = threshold_for_far(&genuine, &impostor, 0.001).unwrap();
    assert!(strict.far <= 0.001 && strict.frr < 0.01, "{strict:?}");
    let r = rates_at(&[0.2, 0.8], &[0.1, 0.5], 0.5);
    assert_eq!((r.far, r.frr), (0.5, 0.5));
    let rep = report(&genuine, &impostor, 0.45, 0.62);
    assert!(rep.f5_07_ok && rep.f5_08_ok && rep.sufficient_impostors);
    assert_eq!(rep.at_config.len(), 2);
    let small = report(&genuine[..10], &impostor[..10], 0.45, 0.62);
    assert!(!small.sufficient_impostors);
    assert!(eer(&[], &impostor).is_none());
    assert!(threshold_for_far(&genuine, &[], 0.01).is_none());
    assert!(eer(&[f32::NAN, 0.9], &[0.1]).is_some(), "NaN pomijane");
}

#[test]
fn thresholds_decisions_and_confidence() {
    let c = SpeakerCfg::default();
    assert!(c.validate().is_ok());
    assert_eq!(c.decide(0.9), Decision::Verified);
    assert_eq!(c.decide(0.5), Decision::Likely);
    assert_eq!(c.decide(0.1), Decision::Rejected);
    assert_eq!(c.decide(f32::NAN), Decision::Rejected);
    assert!((c.confidence(c.threshold_standard) - 0.5).abs() < 1e-4);
    assert!((c.confidence(c.threshold_strict) - 0.999).abs() < 1e-3);
    assert!(c.confidence(0.0) < 0.01 && c.confidence(f32::NAN) == 0.0);
    assert_eq!(c.threshold_for(RiskLevel::Low), c.threshold_standard);
    for r in [RiskLevel::Medium, RiskLevel::High, RiskLevel::Critical] {
        assert_eq!(c.threshold_for(r), c.threshold_strict);
    }
    let bad = SpeakerCfg {
        threshold_strict: 0.3,
        ..SpeakerCfg::default()
    };
    assert!(bad.validate().is_err());
    let v = Verification {
        score: 0.5,
        confidence: c.confidence(0.5),
        decision: c.decide(0.5),
        audio_ms: 2_000,
        model: "m".into(),
    };
    assert!(v.accepts(RiskLevel::Low, &c) && !v.accepts(RiskLevel::High, &c));
    assert_eq!(v.score_permille(), 500);
}

#[test]
fn origin_uses_existing_risk_fields() {
    let verified = SpeakerCheck::Checked {
        decision: Decision::Verified,
        score_permille: 700,
    };
    let likely = SpeakerCheck::Checked {
        decision: Decision::Likely,
        score_permille: 500,
    };
    for (check, expect) in [
        (verified, true),
        (likely, false),
        (SpeakerCheck::Pending, false),
        (SpeakerCheck::NotChecked, false),
    ] {
        match voice_origin(0.92, &check) {
            CommandOrigin::UserVoice {
                confidence,
                speaker_verified,
            } => {
                assert_eq!(speaker_verified, expect, "{check:?}");
                assert_eq!(confidence.permille(), 920);
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(SpeakerCheck::default(), SpeakerCheck::NotChecked);
    let json = serde_json::to_value(verified).unwrap();
    assert_eq!(json["check"], "checked");
}

#[test]
fn embeddings_events_and_consent() {
    let a = Embedding::new(vec![3.0, 4.0]).unwrap();
    assert!((a.as_slice()[0] - 0.6).abs() < 1e-6);
    assert!(Embedding::new(vec![]).is_err() && Embedding::new(vec![0.0, 0.0]).is_err());
    assert!(Embedding::new(vec![f32::NAN]).is_err());
    let b = Embedding::new(vec![4.0, 3.0]).unwrap();
    assert!((cosine(&a, &b) - 0.96).abs() < 1e-5);
    assert!(cosine(&a, &Embedding::new(vec![1.0]).unwrap()).is_nan());
    let m = mean_normalized(&[a.clone(), b]).unwrap();
    assert!((m.as_slice()[0] - m.as_slice()[1]).abs() < 1e-6);
    assert!(mean_normalized(&[]).is_err());
    assert_eq!(format!("{a:?}"), "Embedding(<2 wymiarów>)");
    let mut w = a.clone();
    w.wipe();
    assert!(w.as_slice().iter().all(|x| *x == 0.0));
    assert_eq!(a.clone().into_vec().len(), a.dim());
    assert!(ExportConsent::explicit(5, "laptop").is_valid());
    assert!(!ExportConsent::explicit(5, " ").is_valid());
    let all = [
        SpeakerEvent::EnrollStarted,
        SpeakerEvent::EnrollSample {
            accepted: true,
            done: 1,
            needed: 3,
            reason: None,
        },
        SpeakerEvent::Enrolled {
            utterances: 3,
            model: "m".into(),
        },
        SpeakerEvent::Verified {
            decision: Decision::Likely,
            score_permille: 500,
            audio_ms: 900,
        },
        SpeakerEvent::Deleted,
        SpeakerEvent::Export { granted: false },
    ];
    for e in all {
        assert_eq!(e.to_bus_event().kind.as_str(), e.name());
    }
    assert!(event_schema().is_object());
    for e in [
        SpeakerError::TooShort { ms: 1, min_ms: 2 },
        SpeakerError::Inconsistent { index: 2 },
        SpeakerError::ModelMismatch {
            stored: "a".into(),
            current: "b".into(),
        },
    ] {
        assert!(!e.to_string().is_empty());
    }
}
