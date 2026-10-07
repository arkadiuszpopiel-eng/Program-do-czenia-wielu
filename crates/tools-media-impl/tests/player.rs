//! `SpeakerPlayer` na wirtualnym audio i atrapie schedulera: klip gra po zwolnieniu głośnika
//! (kolejka z mową agentki), wywłaszczenie ścisza i zatrzymuje oraz oddaje głośnik, kill-switch
//! schedulera, anulowanie i `stop_all` zatrzymują od razu, zły format — błąd przed kolejką.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use personas_contract::PersonaId;
use scheduler_lite_contract::{Holder, LeaseRequest, Priority, Resource, SchedulerLite};
use scheduler_lite_fake::FakeScheduler;
use tools_media_contract::{AudioClip, AudioPlayer, CancellationToken, PlayError};
use tools_media_impl::{PlayerConfig, SpeakerPlayer};
use voice_audio_contract::gain::rms;
use voice_audio_contract::synth::sine;
use voice_audio_fake::FakeAudio;

struct Env {
    audio: FakeAudio,
    sched: Arc<FakeScheduler>,
    player: SpeakerPlayer,
    pump: tokio::task::JoinHandle<()>,
}

fn env() -> Env {
    let audio = FakeAudio::new();
    let sched = Arc::new(FakeScheduler::new());
    let player = SpeakerPlayer::new(
        Arc::new(audio.clone()),
        sched.clone(),
        PlayerConfig::default(),
    );
    let clock = audio.clone();
    let pump = tokio::spawn(async move {
        loop {
            clock.advance(Duration::from_millis(10));
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });
    Env {
        audio,
        sched,
        player,
        pump,
    }
}

fn clip(secs: f32) -> AudioClip {
    AudioClip {
        samples: sine(440.0, 16_000, secs, 0.5),
        sample_rate: 16_000,
        channels: 1,
        agent: "delta".into(),
        label: "test.wav".into(),
    }
}

async fn until(what: &str, mut cond: impl FnMut() -> bool) {
    for _ in 0..2000 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("nie doczekano się: {what}");
}

fn speaker(sched: &FakeScheduler) -> Option<Holder> {
    sched.holder(&Resource::Speaker).map(|i| i.holder)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plays_after_the_agent_finishes_speaking() {
    let e = env();
    let beta = Holder::Persona(PersonaId::new("beta"));
    let speech = e
        .sched
        .acquire(LeaseRequest::new(
            Resource::Speaker,
            beta.clone(),
            Priority::Normal,
            Duration::ZERO,
        ))
        .await
        .unwrap();
    let ticket = e
        .player
        .play(clip(0.3), CancellationToken::new())
        .await
        .unwrap();
    assert!(ticket.queued && ticket.duration_ms == 300);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        rms(&e.audio.recorded_output()),
        0.0,
        "czeka na koniec mowy agentki"
    );
    drop(speech);
    until("odtworzenie", || e.player.active() == 0).await;
    assert!(rms(&e.audio.recorded_output()) > 0.05);
    assert_eq!(speaker(&e.sched), None, "głośnik zwolniony");
    e.pump.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preemption_ducks_stops_and_hands_over_the_speaker() {
    let e = env();
    e.player
        .play(clip(5.0), CancellationToken::new())
        .await
        .unwrap();
    until("start", || speaker(&e.sched).is_some()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let gamma = Holder::Persona(PersonaId::new("gamma"));
    let reply = e.sched.acquire(LeaseRequest::new(
        Resource::Speaker,
        gamma.clone(),
        Priority::Normal,
        Duration::from_secs(5),
    ));
    let lease = tokio::time::timeout(Duration::from_secs(3), reply)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.holder(), &gamma, "odpowiedź agentki dostała głośnik");
    until("zatrzymanie", || e.player.active() == 0).await;
    e.pump.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_switch_cancel_and_stop_all() {
    let e = env();
    e.player
        .play(clip(5.0), CancellationToken::new())
        .await
        .unwrap();
    until("start", || speaker(&e.sched).is_some()).await;
    assert!(e.sched.kill_all() >= 1);
    until("kill-switch", || e.player.active() == 0).await;
    let cancel = CancellationToken::new();
    e.player.play(clip(5.0), cancel.clone()).await.unwrap();
    until("start 2", || speaker(&e.sched).is_some()).await;
    cancel.cancel();
    until("anulowanie", || {
        e.player.active() == 0 && speaker(&e.sched).is_none()
    })
    .await;
    e.player
        .play(clip(5.0), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(e.player.stop_all(), 1);
    until("stop_all", || speaker(&e.sched).is_none()).await;
    e.pump.abort();
}

#[tokio::test]
async fn bad_clips_are_rejected_before_queueing() {
    let e = env();
    let mut odd = clip(0.1);
    odd.sample_rate = 12_345;
    assert!(matches!(
        e.player.play(odd, CancellationToken::new()).await,
        Err(PlayError::Format(_))
    ));
    let mut empty = clip(0.1);
    empty.samples.clear();
    assert!(
        e.player
            .play(empty, CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(e.player.active(), 0);
    e.audio
        .fail_next_open(voice_audio_contract::AudioError::NoDefaultDevice);
    assert!(matches!(
        e.player.play(clip(0.1), CancellationToken::new()).await,
        Err(PlayError::Unavailable(_))
    ));
    e.pump.abort();
}
