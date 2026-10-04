//! Weryfikacja właściciela w aplikacji (F5): kreator rejestracji (nagrania z mikrofonu, wskaźnik
//! jakości, profil po ≥ 3 frazach), a w rozmowie — wynik weryfikacji trafia do pochodzenia tury
//! (`CommandOrigin::UserVoice`): obcy głos przy akcji ryzykownej → potwierdzenie nie-głosem,
//! właściciel → zgodnie z poziomem autonomii; destrukcja głosem zawsze pyta nie-głosem (Jądro).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{SampleQuality, SpeakerAction, SpeakerDecisionView, SpeakerState};
use app_api::ports::{VoicePort, VoiceTurnOrigin};
use common::{App, RATE, World, app, speech, timeline};
use risk_classifier_contract::{
    ActionClass, ActionFacts, AutonomyLevel, CommandOrigin, Destructiveness, RiskPolicy,
    SttConfidence, Verdict, evaluate,
};

const OWNER_F0: f32 = 120.0;
const STRANGER_F0: f32 = 300.0;

/// Nagrywa jedną frazę kreatora (mikrofon: mowa `ms` od 100 ms).
async fn record(a: &App, f0: f32, ms: u64, seed: u64) {
    a.world
        .mic(&timeline(ms + 600, &[(100, speech(f0, ms, seed, RATE))]));
    a.voice.speaker(SpeakerAction::RecordStart).await.unwrap();
    let until = a.now() + ms + 300;
    a.until("nagranie frazy", |a| a.now() >= until).await;
    a.voice.speaker(SpeakerAction::RecordStop).await.unwrap();
}

async fn enroll(a: &App) {
    a.voice.speaker(SpeakerAction::Begin).await.unwrap();
    for seed in 1..=3 {
        record(a, OWNER_F0, 2_500, seed).await;
        let v = a.view().await;
        let sample = v.speaker.last_sample.unwrap();
        assert!(sample.accepted, "{sample:?}");
        assert_eq!(sample.quality, SampleQuality::Good);
        assert!(sample.duration_ms >= 2_000 && sample.level_db > -50.0);
    }
    let v = a.voice.speaker(SpeakerAction::Finish).await.unwrap();
    assert_eq!(v.speaker.state, SpeakerState::Enrolled);
}

/// Jedna tura rozmowy (przełącznik) głosem o tonie `f0`; zwraca pochodzenie tury.
async fn voice_turn(a: &App, f0: f32) -> VoiceTurnOrigin {
    a.world.stt.script("wyślij raport na serwer");
    a.world
        .mic(&timeline(6_000, &[(1_000, speech(f0, 2_000, 9, RATE))]));
    a.voice.set_mic_enabled(true).await.unwrap();
    a.until("tura z mowy", |a| !a.turns().is_empty()).await;
    a.voice.set_mic_enabled(false).await.unwrap();
    a.turns()[0].origin
}

fn facts(origin: VoiceTurnOrigin) -> ActionFacts {
    ActionFacts::new("tools-net.post", ActionClass::Egress)
        .egress("example.com", true)
        .origin(CommandOrigin::UserVoice {
            confidence: SttConfidence::from_permille(origin.stt_confidence_permille.unwrap()),
            speaker_verified: origin.speaker_verified,
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn enrollment_wizard_reports_quality_and_builds_a_profile() {
    let a = app(World::new());
    let v = a.view().await;
    assert_eq!(v.speaker.state, SpeakerState::NotEnrolled);
    assert!(v.speaker.required_for_risky, "domyślnie wymagana");
    assert!(v.speaker.prompts.len() >= 3);
    a.voice.speaker(SpeakerAction::Begin).await.unwrap();
    // Za krótka fraza — odrzucona z powodem.
    record(&a, OWNER_F0, 500, 7).await;
    let v = a.view().await;
    let sample = v.speaker.last_sample.unwrap();
    assert!(!sample.accepted);
    assert_eq!(sample.quality, SampleQuality::TooShort);
    assert!(sample.message.is_some());
    assert_eq!(v.speaker.state, SpeakerState::Enrolling);
    assert_eq!(v.speaker.done, 0);
    // Za mało fraz — zakończenie odmawia.
    assert!(a.voice.speaker(SpeakerAction::Finish).await.is_err());
    enroll(&a).await;
    // Usunięcie profilu (crypto-shredding w impl) — stan „niezarejestrowany”.
    let v = a.voice.speaker(SpeakerAction::Delete).await.unwrap();
    assert_eq!(v.speaker.state, SpeakerState::NotEnrolled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stranger_voice_is_rejected_for_risky_actions() {
    let a = app(World::new());
    enroll(&a).await;
    let origin = voice_turn(&a, STRANGER_F0).await;
    assert!(!origin.speaker_verified, "obcy głos");
    let v = a.view().await;
    let check = v.speaker.last_check.unwrap();
    assert_eq!(check.decision, SpeakerDecisionView::Rejected);
    let verdict = evaluate(&facts(origin), AutonomyLevel::L4, &RiskPolicy::default());
    assert!(
        matches!(
            verdict.verdict,
            Verdict::Ask {
                non_voice: true,
                ..
            }
        ),
        "{verdict:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn owner_voice_follows_autonomy_but_destruction_still_asks_non_voice() {
    let a = app(World::new());
    enroll(&a).await;
    let origin = voice_turn(&a, OWNER_F0).await;
    assert!(origin.speaker_verified, "właściciel zweryfikowany");
    assert_eq!(origin.stt_confidence_permille, Some(900));
    let verdict = evaluate(&facts(origin), AutonomyLevel::L4, &RiskPolicy::default());
    assert_eq!(verdict.verdict, Verdict::Proceed, "{verdict:?}");
    let delete = ActionFacts::new("tools-fs.delete", ActionClass::Write)
        .destructive(Destructiveness::Recoverable)
        .origin(CommandOrigin::UserVoice {
            confidence: SttConfidence::from_permille(900),
            speaker_verified: true,
        });
    let verdict = evaluate(&delete, AutonomyLevel::L4, &RiskPolicy::default());
    assert!(matches!(
        verdict.verdict,
        Verdict::Ask {
            non_voice: true,
            ..
        }
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disabling_verification_makes_every_voice_turn_unverified() {
    let a = app(World::new());
    enroll(&a).await;
    let v = a
        .voice
        .speaker(SpeakerAction::SetRequired { required: false })
        .await
        .unwrap();
    assert!(!v.speaker.required_for_risky);
    let origin = voice_turn(&a, OWNER_F0).await;
    assert!(
        !origin.speaker_verified,
        "bez weryfikacji — nawet właściciel"
    );
    let verdict = evaluate(&facts(origin), AutonomyLevel::L4, &RiskPolicy::default());
    assert!(matches!(
        verdict.verdict,
        Verdict::Ask {
            non_voice: true,
            ..
        }
    ));
}
