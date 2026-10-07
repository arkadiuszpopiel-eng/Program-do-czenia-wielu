//! Weryfikacja mówcy w potoku (F5): tura głosowa właściciela → `ReplyRequest::voice` z pewnością
//! STT i wynikiem `Verified` (od razu albo przez `speaker_checked`), obcy głos → `Rejected` →
//! `CommandOrigin::UserVoice { speaker_verified: false }` (akcje ryzykowne: potwierdzenie
//! nie-głosem), bez weryfikatora → `NotChecked`; tura tekstowa bez `voice`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{Opts, RATE, Timeline, World};
use risk_classifier_contract::CommandOrigin;
use voice_audio_contract::synth::{SpeechParams, synthetic_speech};
use voice_dialog_contract::TurnSource;
use voice_pipeline_contract::{PipelineInput, VoicePipeline};
use voice_speaker_contract::{Decision, SpeakerCheck, SpeakerVerifier};
use voice_speaker_fake::{MemoryProfileStore, fake_speaker};

fn speech(f0: f32, ms: u64, seed: u64, rate: u32) -> Vec<f32> {
    synthetic_speech(
        rate,
        ms as f32 / 1000.0,
        SpeechParams {
            f0,
            syllable_rate: 4.0,
            amp: 0.6,
            seed,
        },
    )
}

fn enrolled() -> Arc<dyn SpeakerVerifier> {
    let v = fake_speaker(MemoryProfileStore::new()).unwrap();
    v.begin_enrollment().unwrap();
    for seed in 1..=3 {
        v.add_enrollment(&speech(120.0, 2_500, seed, 16_000))
            .unwrap();
    }
    v.finish_enrollment().unwrap();
    Arc::new(v)
}

fn timeline(f0: f32) -> Timeline {
    let mut t = Timeline::new(8_000, 5);
    let per = RATE as usize / 1000;
    for (i, v) in speech(f0, 2_000, 9, RATE).iter().enumerate() {
        t.samples[1_000 * per + i] += v;
    }
    t.utterances.push((1_000, 3_000));
    t
}

/// Wynik weryfikacji tury 1: z żądania albo z późniejszego `speaker_checked` (wątek roboczy
/// biegnie w czasie rzeczywistym — czekamy najwyżej 3 s).
async fn final_check(w: &mut World) -> SpeakerCheck {
    for _ in 0..300 {
        let from_request = w.slow_reply.requests.lock().unwrap()[0]
            .voice
            .map(|v| v.speaker);
        if let Some(c) = from_request.filter(|c| *c != SpeakerCheck::Pending) {
            return c;
        }
        if let Some((_, c)) = w.slow_reply.checks.lock().unwrap().first() {
            return *c;
        }
        std::thread::sleep(Duration::from_millis(10));
        w.tick().await;
    }
    panic!("brak wyniku weryfikacji");
}

async fn run_turn(f0: f32, verifier: Option<Arc<dyn SpeakerVerifier>>) -> (World, SpeakerCheck) {
    let mut w = World::new(Opts::default());
    w.mic(&timeline(f0));
    if let Some(v) = verifier {
        w.p.set_speaker_verifier(v).unwrap();
    }
    w.stt.script("usuń stare kopie");
    w.answer(&["Dobrze."]);
    w.conversation_mode().await;
    w.run_until(6_000).await;
    let req = w.slow_reply.requests.lock().unwrap()[0].clone();
    assert_eq!(req.source, TurnSource::Voice);
    let voice = req.voice.expect("tura głosowa ma pochodzenie");
    assert_eq!(voice.stt_confidence_permille, 900);
    let check = final_check(&mut w).await;
    (w, check)
}

#[tokio::test]
async fn owner_turn_is_verified_and_stranger_is_not() {
    let (_, owner) = run_turn(120.0, Some(enrolled())).await;
    assert!(
        matches!(
            owner,
            SpeakerCheck::Checked {
                decision: Decision::Verified,
                ..
            }
        ),
        "{owner:?}"
    );
    let (w, stranger) = run_turn(300.0, Some(enrolled())).await;
    assert!(
        matches!(
            stranger,
            SpeakerCheck::Checked {
                decision: Decision::Rejected,
                ..
            }
        ),
        "{stranger:?}"
    );
    let mut voice = w.slow_reply.requests.lock().unwrap()[0].voice.unwrap();
    voice.speaker = stranger;
    assert!(matches!(
        voice.command_origin(),
        CommandOrigin::UserVoice {
            speaker_verified: false,
            ..
        }
    ));
}

#[tokio::test]
async fn without_verifier_turn_is_unverified_and_text_has_no_voice() {
    let (mut w, check) = run_turn(120.0, None).await;
    assert_eq!(check, SpeakerCheck::NotChecked);
    let origin = w.slow_reply.requests.lock().unwrap()[0]
        .voice
        .unwrap()
        .command_origin();
    assert!(matches!(
        origin,
        CommandOrigin::UserVoice {
            speaker_verified: false,
            ..
        }
    ));
    w.answer(&["Jasne."]);
    w.p.input(PipelineInput::Typed {
        text: "a teraz tekstem".into(),
    });
    w.run_until(7_500).await;
    let reqs = w.slow_reply.requests.lock().unwrap().clone();
    let typed = reqs.last().unwrap();
    assert_eq!((typed.source, typed.voice), (TurnSource::Text, None));
}
