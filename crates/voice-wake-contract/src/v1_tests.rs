//! Testy v1: frazy, detektor z histerezą, nasłuch z buforem (prywatność), sesja, FAR/FRR.

use personas_contract::{Catalog, PersonaId, builtin_personas};
use voice_audio_contract::synth::{SpeechParams, sine, synthetic_speech, white_noise};

use crate::eval::{ScoredItem, ScoredKind, default_thresholds, evaluate, recommend, sweep};
use crate::*;

fn cfg() -> WakeWordCfg {
    WakeWordCfg::from_personas(&builtin_personas(), 0.8)
}

fn labels() -> Vec<String> {
    ["hej alfa", "hej beta", "hej gama", "hej delta"]
        .map(String::from)
        .to_vec()
}

fn scores(at_ms: u64, v: [f32; 4]) -> KwsScores {
    KwsScores {
        at_ms,
        scores: v.to_vec(),
    }
}

#[test]
fn phrases_validation() {
    let c = cfg();
    assert_eq!(c.phrases.len(), 4);
    assert!(c.validate().is_ok());
    assert_eq!(syllables("Hej Alfa"), 3);
    assert_eq!(syllables("Hej, Delto!"), 3);
    assert_eq!(syllables("Hej Ew"), 2);
    assert_eq!(normalize_phrase("  Hej,  GAMA! "), "hej gama");
    let short = WakeWordCfg {
        phrases: vec![("Hej Ew".into(), PersonaId::alfa())],
        ..cfg()
    };
    assert!(matches!(short.validate(), Err(WakeError::InvalidConfig(_))));
    let dup = WakeWordCfg {
        phrases: vec![
            ("Hej Alfa".into(), PersonaId::alfa()),
            ("hej alfa!".into(), PersonaId::beta()),
        ],
        ..cfg()
    };
    assert!(dup.validate().is_err());
    for t in [0.0, 1.0, f32::NAN] {
        assert!(
            WakeWordCfg {
                threshold: t,
                ..cfg()
            }
            .validate()
            .is_err()
        );
    }
    let always = WakeWordCfg {
        always_on: true,
        owner_gate: true,
        ..cfg()
    };
    assert!(matches!(always.validate(), Err(WakeError::NotAvailable(_))));
    let on = WakeCfg {
        wake_words: Some(cfg()),
        ..WakeCfg::default()
    };
    assert!(on.validate().is_ok());
    assert_eq!(
        cfg().persona_for("HEJ DELTA").map(|(_, p)| p.clone()),
        Some(PersonaId::delta())
    );
    assert!(cfg().persona_for("hej zeta").is_none());
    assert!(KwsParams::default().validate().is_ok());
    for bad in [
        KwsParams {
            hysteresis: 0.7,
            ..KwsParams::default()
        },
        KwsParams {
            min_hits: 0,
            ..KwsParams::default()
        },
        KwsParams {
            ring_ms: 100,
            ..KwsParams::default()
        },
        KwsParams {
            gate_preroll_ms: 3_000,
            ..KwsParams::default()
        },
        KwsParams {
            listen_timeout_ms: 10,
            ..KwsParams::default()
        },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
}

#[test]
fn detector_hysteresis_and_refractory() {
    let mut d = WakeWordDetector::new(&cfg(), &labels(), KwsParams::default()).unwrap();
    assert_eq!(d.mapped_labels(), 4);
    assert!(d.push(&scores(0, [0.1, 0.0, 0.0, 0.0])).is_none());
    assert!(
        d.push(&scores(80, [0.9, 0.0, 0.0, 0.0])).is_none(),
        "1 trafienie"
    );
    let hit = d.push(&scores(160, [0.95, 0.0, 0.0, 0.0])).unwrap();
    assert_eq!(
        (hit.persona.clone(), hit.phrase.as_str()),
        (PersonaId::alfa(), "Hej Alfa")
    );
    assert_eq!(hit.score_permille(), 950);
    // Wysoko dalej / spadek do strefy histerezy — bez ponownego uzbrojenia.
    for (t, v) in [(240, 0.9), (320, 0.7), (400, 0.9), (480, 0.95)] {
        assert!(d.push(&scores(t, [v, 0.0, 0.0, 0.0])).is_none(), "t={t}");
    }
    // Uzbrojenie, ale w oknie odporności (2 s od 160 ms).
    d.push(&scores(560, [0.1, 0.0, 0.0, 0.0]));
    for t in [640, 720] {
        assert!(d.push(&scores(t, [0.0, 0.0, 0.0, 0.9])).is_none());
    }
    d.push(&scores(2_100, [0.0; 4]));
    d.push(&scores(2_200, [0.0, 0.0, 0.0, 0.85]));
    let hit = d.push(&scores(2_280, [0.0, 0.0, 0.0, 0.9])).unwrap();
    assert_eq!(hit.persona, PersonaId::delta());
    // Pojedynczy pik nie wystarcza; NaN = 0.
    d.reset();
    assert!(d.push(&scores(0, [f32::NAN, 0.99, 0.0, 0.0])).is_none());
    assert!(d.push(&scores(80, [0.0, 0.1, 0.0, 0.0])).is_none());
    // Dwie etykiety naraz — wygrywa wyższy wynik.
    d.push(&scores(160, [0.85, 0.95, 0.0, 0.0]));
    let hit = d.push(&scores(240, [0.85, 0.95, 0.0, 0.0])).unwrap();
    assert_eq!(hit.persona, PersonaId::beta());
    let unknown = WakeWordDetector::new(&cfg(), &["ok google".into()], KwsParams::default());
    assert!(unknown.is_err());
}

/// Model testowy: wynik = udział energii tonu 1 kHz (etykieta „hej delta”) w ramce 80 ms.
struct ToneScorer {
    labels: Vec<String>,
    buf: Vec<f32>,
    t_ms: u64,
    fed: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl KeywordScorer for ToneScorer {
    fn labels(&self) -> &[String] {
        &self.labels
    }

    fn push(&mut self, samples: &[f32]) -> Result<Vec<KwsScores>, WakeError> {
        self.fed
            .fetch_add(samples.len(), std::sync::atomic::Ordering::SeqCst);
        self.buf.extend_from_slice(samples);
        let mut out = Vec::new();
        while self.buf.len() >= 1_280 {
            let w: Vec<f32> = self.buf.drain(..1_280).collect();
            self.t_ms += 80;
            let (mut c, mut s, mut e) = (0.0f32, 0.0f32, 0.0f32);
            for (i, x) in w.iter().enumerate() {
                let a = 2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / 16_000.0;
                c += x * a.cos();
                s += x * a.sin();
                e += x * x;
            }
            let ratio = (2.0 * (c * c + s * s) / 1_280.0) / e.max(1e-9);
            out.push(scores(self.t_ms, [0.0, 0.0, 0.0, ratio.clamp(0.0, 1.0)]));
        }
        Ok(out)
    }

    fn reset(&mut self) {
        self.buf.clear();
        self.t_ms = 0;
    }
}

fn listener(
    owner_gate: bool,
) -> (
    WakeWordListener,
    std::sync::Arc<std::sync::atomic::AtomicUsize>,
) {
    let fed = std::sync::Arc::default();
    let scorer = ToneScorer {
        labels: labels(),
        buf: Vec::new(),
        t_ms: 0,
        fed: std::sync::Arc::clone(&fed),
    };
    let c = WakeWordCfg {
        owner_gate,
        ..cfg()
    };
    (
        WakeWordListener::new(&c, KwsParams::default(), Box::new(scorer)).unwrap(),
        fed,
    )
}

#[test]
fn listener_keeps_audio_inside_until_detection() {
    let (mut l, fed) = listener(false);
    let quiet = white_noise(1, 16_000 * 5, 0.001);
    assert!(l.push(&quiet).unwrap().is_none());
    assert_eq!(
        fed.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "cisza: model nie dostaje audio"
    );
    for seed in 0..12 {
        let speech = synthetic_speech(
            16_000,
            5.0,
            SpeechParams {
                seed,
                ..SpeechParams::default()
            },
        );
        assert!(l.push(&speech).unwrap().is_none());
        assert!(l.retained_samples() <= l.capacity_samples());
    }
    assert!(l.stats().scored_frames > 0 && l.stats().triggers == 0);
    assert!(!format!("{l:?}").contains("0.0"), "Debug bez próbek");
    let tone = sine(1_000.0, 16_000, 0.6, 0.3);
    let t = l.push(&tone).unwrap().expect("wykrycie tonu");
    assert_eq!(t.hit.persona, PersonaId::delta());
    assert!(t.audio.len() <= 32_000 && !t.audio.is_empty());
    assert!(format!("{t:?}").contains("audio_samples"));
    assert_eq!(l.retained_samples(), 0, "po wykryciu bufor pusty");
    l.set_suspended(true);
    let before = fed.load(std::sync::atomic::Ordering::SeqCst);
    assert!(l.push(&sine(1_000.0, 16_000, 3.0, 0.3)).unwrap().is_none());
    assert_eq!(fed.load(std::sync::atomic::Ordering::SeqCst), before);
    assert_eq!(l.retained_samples(), 0);
    assert!(l.is_suspended() && l.stats().suspended_frames >= 299);
}

struct Owner(Option<bool>);
impl OwnerCheck for Owner {
    fn is_owner(&mut self, audio: &[f32]) -> Option<bool> {
        assert!(!audio.is_empty());
        self.0
    }
}

#[test]
fn owner_gate_is_fail_closed() {
    // Tło przed frazą (bramka energii uczy się szumu od pierwszej ramki).
    let mut tone = white_noise(2, 8_000, 0.001);
    tone.extend(sine(1_000.0, 16_000, 0.6, 0.3));
    let (mut l, _) = listener(true);
    assert!(
        l.push(&tone).unwrap().is_none(),
        "bez sprawdzenia — odrzucone"
    );
    assert_eq!(l.stats().owner_rejected, 1);
    for (owner, expect) in [(None, false), (Some(false), false), (Some(true), true)] {
        let (l, _) = listener(true);
        let mut l = l.with_owner_check(Box::new(Owner(owner)));
        assert_eq!(l.push(&tone).unwrap().is_some(), expect, "{owner:?}");
    }
}

#[test]
fn machine_wake_word_session() {
    let cast = Catalog::builtin().default_cast(true);
    let mut m = WakeMachine::new(builtin_personas(), Some(cast));
    let wake = |at_ms| WakeInput::WakeWord {
        persona: PersonaId::delta(),
        phrase: "Hej Delta".into(),
        at_ms,
    };
    let ev = m.handle(wake(100));
    assert!(matches!(
        ev.as_slice(),
        [WakeEvent::WakeWordIgnored {
            reason: WakeIgnoreReason::Disabled,
            ..
        }]
    ));
    m.set_wake_words(true, 3_000);
    assert!(m.wake_words_enabled());
    let ev = m.handle(wake(200));
    assert!(ev.contains(&WakeEvent::ListenStart {
        addressed: Some(PersonaId::delta()),
        source: WakeSource::WakeWord
    }));
    assert!(ev.contains(&WakeEvent::Addressed {
        persona: PersonaId::delta(),
        by_name: true
    }));
    assert_eq!(m.last_addressed(), Some(&PersonaId::delta()));
    assert!(matches!(
        m.handle(wake(300)).as_slice(),
        [WakeEvent::WakeWordIgnored {
            reason: WakeIgnoreReason::AlreadyListening,
            ..
        }]
    ));
    assert!(m.handle(WakeInput::Tick { now_ms: 3_100 }).is_empty());
    let ev = m.handle(WakeInput::Tick { now_ms: 3_200 });
    assert!(ev.contains(&WakeEvent::ListenStop {
        source: WakeSource::WakeWord
    }));
    assert!(ev.iter().any(|e| e.name() == EVENT_FALSE_ALARM));
    // Z mową: koniec po ciszy, bez podejrzenia fałszywego alarmu.
    m.handle(wake(10_000));
    m.handle(WakeInput::Tick { now_ms: 10_500 });
    m.handle(WakeInput::Vad { speech: true });
    m.handle(WakeInput::Tick { now_ms: 20_000 });
    assert!(m.listening().is_some(), "trwa mowa");
    m.handle(WakeInput::Vad { speech: false });
    m.handle(WakeInput::Processing { busy: true });
    assert!(m.handle(WakeInput::Tick { now_ms: 40_000 }).is_empty());
    m.handle(WakeInput::Processing { busy: false });
    let ev = m.handle(WakeInput::Tick { now_ms: 43_000 });
    assert!(ev.contains(&WakeEvent::ListenStop {
        source: WakeSource::WakeWord
    }));
    assert!(ev.iter().all(|e| e.name() != EVENT_FALSE_ALARM));
    m.handle(WakeInput::SetDnd { on: true });
    assert!(matches!(
        m.handle(wake(50_000)).last(),
        Some(WakeEvent::WakeWordIgnored {
            reason: WakeIgnoreReason::DoNotDisturb,
            ..
        })
    ));
    m.handle(WakeInput::SetDnd { on: false });
    m.handle(WakeInput::SetMuted { muted: true });
    assert!(matches!(
        m.handle(wake(51_000)).last(),
        Some(WakeEvent::WakeWordIgnored {
            reason: WakeIgnoreReason::Muted,
            ..
        })
    ));
    for e in [
        WakeEvent::FalseAlarmSuspected {
            persona: PersonaId::alfa(),
            phrase: "Hej Alfa".into(),
        },
        WakeEvent::WakeWordIgnored {
            persona: PersonaId::alfa(),
            reason: WakeIgnoreReason::Disabled,
        },
    ] {
        assert_eq!(e.to_bus_event().kind.as_str(), e.name());
    }
}

#[test]
fn far_frr_metrics_and_sweep() {
    let pos = |id: &str, peak: f32| ScoredItem {
        id: id.into(),
        kind: ScoredKind::Positive {
            persona: PersonaId::gama(),
            window: Some((0, 1_000)),
        },
        duration_ms: 2_000,
        scores: (0..10)
            .map(|i| scores(i * 80, [0.0, 0.0, if i >= 4 { peak } else { 0.0 }, 0.0]))
            .collect(),
    };
    let bg = ScoredItem {
        id: "tlo".into(),
        kind: ScoredKind::Background,
        duration_ms: 12 * 3_600_000,
        scores: (0..20)
            .map(|i| scores(i * 80, [if (5..8).contains(&i) { 0.72 } else { 0.0 }; 4]))
            .collect(),
    };
    let items = vec![pos("p1", 0.9), pos("p2", 0.75), bg];
    let p = evaluate(&items, &cfg(), &labels(), KwsParams::default(), 0.8).unwrap();
    assert_eq!((p.positives, p.detected, p.false_alarms), (2, 1, 0));
    assert_eq!(p.missed, vec!["p2".to_owned()]);
    assert!((p.frr() - 0.5).abs() < 1e-9 && p.far_per_day() == 0.0);
    assert!(!p.passes() && !p.sufficient());
    let points = sweep(
        &items,
        &cfg(),
        &labels(),
        KwsParams::default(),
        &default_thresholds(),
    )
    .unwrap();
    assert_eq!(points.len(), 14);
    let at70 = points
        .iter()
        .find(|p| (p.threshold - 0.7).abs() < 1e-3)
        .unwrap();
    assert_eq!((at70.detected, at70.false_alarms), (2, 1));
    assert!((at70.far_per_day() - 2.0).abs() < 1e-9);
    let best = recommend(&points).unwrap();
    assert!(best.far_per_day() <= 1.0);
}
