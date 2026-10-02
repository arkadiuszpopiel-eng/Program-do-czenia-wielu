//! Runner audio na atrapach (wirtualny zegar): PTT — jedna wypowiedź na przytrzymanie,
//! przełącznik — wypowiedzi z VAD i „koniec dyktowania”, mikrofon jako zasób wyłączny
//! (zajęty → odmowa, zwalniany z sesją).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::Desk;
use platform_contract::WindowId;
use scheduler_lite_contract::{Holder, LeaseRequest, Priority, Resource, SchedulerLite};
use scheduler_lite_fake::FakeScheduler;
use voice_audio_contract::synth::{SpeechParams, synthetic_speech, white_noise};
use voice_audio_fake::FakeAudio;
use voice_dictation_contract::contract_tests::DesktopDriver;
use voice_dictation_contract::{Dictation, DictationMode, DictationPhase};
use voice_dictation_impl::{DictationAudio, DictationRunner, DictationService};
use voice_stt_fake::FakeStt;
use voice_vad_fake::FakeVad;

struct Rig {
    audio: FakeAudio,
    stt: Arc<FakeStt>,
    sched: Arc<FakeScheduler>,
    desk: Desk,
    runner: DictationRunner<DictationService>,
    now: u64,
}

/// Mikrofon: cisza + wypowiedzi `(start_ms, ms)`.
fn rig(utterances: &[(u64, u64)], total_ms: u64) -> Rig {
    let audio = FakeAudio::new();
    let mut mic = white_noise(1, total_ms as usize * 16, 0.0005);
    for (i, (at, ms)) in utterances.iter().enumerate() {
        let s = synthetic_speech(
            16_000,
            *ms as f32 / 1000.0,
            SpeechParams {
                seed: i as u64 + 1,
                ..SpeechParams::default()
            },
        );
        for (k, v) in s.iter().enumerate() {
            mic[*at as usize * 16 + k] += v;
        }
    }
    audio.set_mic_signal(&mic, 16_000, false);
    let stt = Arc::new(FakeStt::new());
    let sched = Arc::new(FakeScheduler::new());
    let desk = Desk::new();
    let parts = DictationAudio {
        audio: Arc::new(audio.clone()),
        vad: Box::new(FakeVad::new()),
        stt: stt.clone(),
        scheduler: sched.clone(),
    };
    let runner = DictationRunner::new(parts, desk.service());
    Rig {
        audio,
        stt,
        sched,
        desk,
        runner,
        now: 0,
    }
}

impl Rig {
    async fn run_until(&mut self, ms: u64) {
        while self.now < ms {
            self.now += 10;
            self.audio.advance(Duration::from_millis(10));
            self.sched.advance(10);
            self.runner.step(self.now).await.unwrap();
        }
    }
}

#[tokio::test]
async fn push_to_talk_types_one_utterance() {
    let mut r = rig(&[(300, 1_500)], 3_000);
    r.stt.script("ala ma kota kropka");
    let editor = WindowId(r.desk.open_app("notepad.exe"));
    r.runner.begin(DictationMode::PushToTalk, 0).await.unwrap();
    assert!(r.runner.mic_open());
    assert_eq!(
        r.sched.holder(&Resource::Mic).map(|l| l.holder),
        Some(Holder::User)
    );
    r.run_until(2_000).await;
    assert_eq!(r.desk.text(editor.0), "", "PTT: wpis dopiero po puszczeniu");
    r.runner.end(2_000).await.unwrap();
    assert_eq!(r.desk.replay(editor), "Ala ma kota.");
    assert!(!r.runner.mic_open() && r.audio.open_inputs() == 0);
    assert!(
        r.sched.holder(&Resource::Mic).is_none(),
        "mikrofon zwolniony"
    );
    assert_eq!(r.runner.dictation().status().phase, DictationPhase::Idle);
}

#[tokio::test]
async fn toggle_mode_types_each_utterance_and_stops_by_voice() {
    let mut r = rig(&[(300, 1_200), (2_500, 1_200), (5_000, 1_000)], 8_000);
    r.stt.script("pierwsze zdanie kropka");
    r.stt.script("drugie zdanie znak zapytania");
    r.stt.script("koniec dyktowania");
    let editor = WindowId(r.desk.open_app("code.exe"));
    r.runner.begin(DictationMode::Toggle, 0).await.unwrap();
    r.run_until(4_500).await;
    assert_eq!(r.desk.replay(editor), "Pierwsze zdanie. Drugie zdanie?");
    r.run_until(7_500).await;
    assert_eq!(r.runner.dictation().status().phase, DictationPhase::Idle);
    assert!(!r.runner.mic_open(), "„koniec dyktowania” zamyka mikrofon");
    assert_eq!(r.desk.replay(editor), "Pierwsze zdanie. Drugie zdanie?");
}

#[tokio::test]
async fn busy_mic_refuses_start() {
    let mut r = rig(&[], 1_000);
    let _held = r
        .sched
        .acquire(LeaseRequest::new(
            Resource::Mic,
            Holder::System("nagrywanie".into()),
            Priority::Critical,
            Duration::ZERO,
        ))
        .await
        .unwrap();
    r.desk.open_app("notepad.exe");
    assert!(r.runner.begin(DictationMode::Toggle, 0).await.is_err());
    assert_eq!(r.runner.dictation().status().phase, DictationPhase::Idle);
    assert!(!r.runner.mic_open());
}
