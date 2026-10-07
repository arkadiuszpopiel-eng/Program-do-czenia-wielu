//! Testy atrapy: kontrakt współdzielony + skrypt na wirtualnym zegarze.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use personas_contract::PersonaId;
use voice_wake_contract::contract_tests::{self, KeyDriver};
use voice_wake_contract::{MicState, Wake, WakeEvent, WakeInput, WakeSource};
use voice_wake_fake::FakeWake;

#[test]
fn contract_suite() {
    contract_tests::run_all(|| {
        let w = FakeWake::new();
        let keys = w.keys();
        (w, Box::new(keys) as Box<dyn KeyDriver>)
    });
}

#[test]
fn scripted_ptt_and_addressing_on_virtual_clock() {
    let mut w = FakeWake::new();
    w.schedule(
        Duration::from_millis(100),
        WakeInput::UiPtt { pressed: true },
    );
    w.schedule(Duration::from_millis(150), WakeInput::Vad { speech: true });
    w.schedule(
        Duration::from_millis(900),
        WakeInput::UiPtt { pressed: false },
    );
    w.schedule(
        Duration::from_millis(1_200),
        WakeInput::Transcript {
            text: "Gamo, sprawdź pocztę".into(),
        },
    );
    assert!(w.advance(Duration::from_millis(99)).is_empty());
    let ev = w.advance(Duration::from_millis(60));
    assert_eq!(
        ev[0],
        WakeEvent::ListenStart {
            addressed: None,
            source: WakeSource::Ui
        }
    );
    assert_eq!(w.mic_state(), MicState::Hearing);
    let ev = w.advance(Duration::from_millis(800));
    assert!(ev.contains(&WakeEvent::ListenStop {
        source: WakeSource::Ui
    }));
    let ev = w.advance(Duration::from_millis(400));
    assert_eq!(
        ev,
        vec![WakeEvent::Addressed {
            persona: PersonaId::gama(),
            by_name: true
        }]
    );
    assert!(w.configured().is_none());
}

#[test]
fn scorers_drive_the_listener() {
    use personas_contract::builtin_personas;
    use voice_wake_contract::{KeywordScorer, KwsParams, WakeWordCfg, WakeWordListener};
    use voice_wake_fake::{ScriptedScorer, ToneScorer};

    let cfg = WakeWordCfg::from_personas(&builtin_personas(), 0.8);
    let mut tone = ToneScorer::builtin();
    let s = tone.push(&vec![0.0; 2_560]).unwrap();
    assert_eq!(s.len(), 2);
    assert_eq!(s[1].at_ms, 160);
    assert_eq!(tone.fed_samples(), 2_560);
    tone.reset();
    let params = KwsParams {
        vad_gate: false,
        ..KwsParams::default()
    };
    let script = ScriptedScorer::new(
        &["hej alfa", "hej beta"],
        80,
        vec![(400, 1, 0.9), (480, 1, 0.95)],
    );
    let mut l = WakeWordListener::new(&cfg, params, Box::new(script)).unwrap();
    let t = l.push(&vec![0.0; 16_000]).unwrap().unwrap();
    assert_eq!(t.hit.persona, PersonaId::beta());
    assert_eq!(t.hit.at_ms, 480);
    assert!(l.push(&vec![0.0; 16_000]).unwrap().is_none());
}
