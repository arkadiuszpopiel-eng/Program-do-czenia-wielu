//! Testy atrap: kontrakt współdzielony na `FakeDialog`, zasób głośnika, klasyfikator, alignment.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_dialog_contract::{
    InterruptClassifier, InterruptContext, InterruptIntent, SpeakerLock, SpeakerOwner, UtteranceId,
    WordAligner, contract_tests,
};
use voice_dialog_fake::{
    FakeDialog, FakeSpeakerLock, LockCall, ScriptedClassifier, UniformAligner,
};
use voice_persona_contract::PersonaId;

#[test]
fn contract_suite() {
    contract_tests::run_all(&FakeDialog);
}

#[test]
fn speaker_lock_is_exclusive() {
    let lock = FakeSpeakerLock::new();
    let alfa = SpeakerOwner {
        persona: PersonaId::alfa(),
        utterance: UtteranceId(1),
    };
    let beta = SpeakerOwner {
        persona: PersonaId::beta(),
        utterance: UtteranceId(2),
    };
    lock.try_acquire(&alfa).unwrap();
    lock.try_acquire(&alfa).unwrap();
    assert_eq!(lock.try_acquire(&beta).unwrap_err().holder, alfa);
    assert!(!lock.release(&beta));
    assert!(lock.release(&alfa));
    assert!(lock.holder().is_none());
    lock.occupy(beta.clone());
    assert!(lock.try_acquire(&alfa).is_err());
    assert!(matches!(lock.log()[2], LockCall::Denied(_)));
}

#[test]
fn scripted_classifier_and_uniform_aligner() {
    let c = ScriptedClassifier::new();
    c.script("Dalej", InterruptIntent::Continue);
    let ctx = |u| InterruptContext {
        heard_prefix: "",
        unsaid: "",
        utterance: u,
    };
    assert_eq!(
        c.classify(&ctx(" dalej ")).intent,
        InterruptIntent::Continue
    );
    assert_eq!(c.classify(&ctx("coś")).intent, InterruptIntent::Correction);
    assert_eq!(c.calls().len(), 2);
    let marks = UniformAligner
        .align("ala ma kota", &vec![0.0; 16_000], 16_000)
        .unwrap();
    assert_eq!(marks.len(), 3);
    assert_eq!((marks[1].char_start, marks[1].char_end), (4, 6));
    assert!(marks[2].end_ms <= 1000 && marks[0].start_ms == 0);
    assert!(UniformAligner.align("x", &[], 0).is_err());
}
