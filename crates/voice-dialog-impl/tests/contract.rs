//! Kontrakt współdzielony, manifest i sterownik z zasobem głośnika.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_registry_contract::ModuleManifest;
use personas_contract::PersonaId;
use voice_dialog_contract::{
    ActivationSource, Command, DialogEvent, DialogNotice, DialogPhase, SpeakerLock, SpeakerOwner,
    UtteranceId, contract_tests,
};
use voice_dialog_fake::{FakeSpeakerLock, LockCall, ScriptedClassifier};
use voice_dialog_impl::{DialogDriver, DialogMachine, MODULE_TOML, default_machine};

#[test]
fn contract_suite() {
    contract_tests::run_all(&default_machine());
    let scripted = DialogMachine::new(
        voice_dialog_contract::DialogConfig::default(),
        ScriptedClassifier::new(),
    );
    contract_tests::run_all(&scripted);
}

#[test]
fn manifest_is_valid() {
    let m = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "voice-dialog");
    assert_eq!(m.provides[0].to_string(), "voice-dialog-contract@1");
}

#[test]
fn driver_serializes_speaker_between_agents() {
    let lock = FakeSpeakerLock::new();
    let other = SpeakerOwner {
        persona: PersonaId::delta(),
        utterance: UtteranceId(999),
    };
    lock.occupy(other.clone());
    let mut d = DialogDriver::new(default_machine(), lock);
    d.handle(
        DialogEvent::Activate {
            source: ActivationSource::PushToTalk,
        },
        0,
    );
    d.handle(DialogEvent::VadSpeechStart, 100);
    d.handle(
        DialogEvent::UserPartial {
            text: "Która godzina?".into(),
        },
        300,
    );
    d.handle(DialogEvent::VadSpeechEnd, 500);
    d.handle(DialogEvent::TurnEnded, 700);
    let cmds = d.handle(
        DialogEvent::ResponseReady {
            persona: PersonaId::alfa(),
        },
        900,
    );
    // Delta mówi w innej sesji — Alfa czeka (jedna agentka naraz).
    assert!(cmds.iter().any(|c| matches!(
        c,
        Command::Notify {
            notice: DialogNotice::SpeakerBusy { .. }
        }
    )));
    assert_eq!(d.state().phase, DialogPhase::Thinking);
    assert!(d.lock().release(&other));
    let cmds = d.handle(DialogEvent::SpeakerReleased, 1500);
    assert!(cmds.iter().any(|c| matches!(c, Command::StartTts { .. })));
    assert_eq!(d.state().phase, DialogPhase::Speaking);
    let id = d.state().utterance.as_ref().unwrap().id;
    assert_eq!(d.lock().holder().unwrap().utterance, id);
    d.handle(DialogEvent::StopSpeech, 1600);
    assert!(d.lock().holder().is_none());
    assert!(
        d.lock()
            .log()
            .iter()
            .any(|l| matches!(l, LockCall::Released(o, true) if o.utterance == id))
    );
}
