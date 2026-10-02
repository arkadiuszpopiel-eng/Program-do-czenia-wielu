//! Słowa wywoławcze w potoku (F5): przed wykryciem audio mikrofonu nie wychodzi poza nasłuch
//! `voice-wake` (0 ramek do VAD/STT, 0 transkryptów, 0 zdarzeń VAD), wykrycie „Hej Delta”
//! (atrapa: ton podpisu) otwiera słuchanie z adresatką Deltą, po ciszy sesja się zamyka i nasłuch
//! wraca; wyciszenie i DND wstrzymują nasłuch; bez `wake_words` w konfiguracji — wykrycie ignorowane.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Opts, RATE, Timeline, World, natural};
use personas_contract::{PersonaId, builtin_personas};
use voice_audio_contract::synth::sine;
use voice_pipeline_contract::{PipelineInput, VoicePipeline};
use voice_wake_contract::{
    EVENT_LISTEN_START, EVENT_LISTEN_STOP, EVENT_WORD_IGNORED, KwsParams, Wake, WakeCfg,
    WakeWordCfg, WakeWordListener,
};
use voice_wake_fake::{FakeWake, ToneScorer};

fn cfg() -> WakeWordCfg {
    WakeWordCfg::from_personas(&builtin_personas(), 0.8)
}

fn listener() -> WakeWordListener {
    WakeWordListener::new(
        &cfg(),
        KwsParams::default(),
        Box::new(ToneScorer::builtin()),
    )
    .unwrap()
}

fn world(enabled: bool) -> World {
    let mut wake = FakeWake::new();
    if enabled {
        wake.configure(WakeCfg {
            wake_words: Some(cfg()),
            ..WakeCfg::default()
        })
        .unwrap();
    }
    World::with_wake(Opts::default(), wake)
}

/// Oś: 8 s rozmów w tle (bez frazy), 0,6 s „Hej Delta”, polecenie 1,5 s, cisza.
fn timeline() -> Timeline {
    let mut t = Timeline::new(20_000, 3);
    let per = RATE as usize / 1000;
    for (at, ms, seed) in [(500u64, 3_000u64, 1u64), (4_000, 3_500, 2)] {
        for (i, v) in natural(ms, seed).iter().enumerate() {
            t.samples[at as usize * per + i] += v * 0.6;
        }
    }
    let tone = sine(1_365.0, RATE, 0.6, 0.3);
    for (i, v) in tone.iter().enumerate() {
        t.samples[9_000 * per + i] += v;
    }
    t.speech(10_000, 1_500, 4);
    t
}

#[tokio::test]
async fn wake_word_opens_listening_and_nothing_leaks_before() {
    let mut w = world(true);
    w.mic(&timeline());
    w.stt.script("jaka będzie pogoda");
    w.answer(&["Słonecznie."]);
    w.p.arm_wake_words(listener()).unwrap();
    assert!(w.p.wake_words_armed() && w.p.status().mic_open);
    w.run_until(8_900).await;
    let stats = w.p.wake_listener_stats().unwrap();
    assert!(stats.scored_frames > 0 && stats.triggers == 0, "{stats:?}");
    for name in [
        "voice.vad.speech_start",
        "voice.pipeline.transcript",
        "voice.stt.partial",
        "voice.stt.final",
        EVENT_LISTEN_START,
    ] {
        assert!(w.events(name).is_empty(), "{name} przed wykryciem");
    }
    assert_eq!(w.stt.pending_script(), 1, "STT nie dostało wypowiedzi");
    assert!(
        w.bus
            .recorded()
            .iter()
            .all(|e| !e.payload.to_string().contains("pcm")),
        "zdarzenia bez audio"
    );
    w.run_until(9_800).await;
    let start = w.events(EVENT_LISTEN_START);
    assert_eq!(start.len(), 1);
    assert_eq!(start[0].payload["source"], "wake_word");
    assert_eq!(start[0].payload["addressed"], "delta");
    assert_eq!(w.p.status().persona, PersonaId::delta());
    w.run_until(19_000).await;
    assert_eq!(
        w.stt.pending_script(),
        0,
        "polecenie po frazie poszło do STT"
    );
    assert!(!w.events("voice.pipeline.transcript").is_empty());
    assert_eq!(
        w.events(EVENT_LISTEN_STOP).len(),
        1,
        "sesja zamknięta po ciszy"
    );
    assert!(
        w.p.status().mic_open && w.p.wake_words_armed(),
        "nasłuch wrócił"
    );
    w.p.disarm_wake_words();
    w.tick().await;
    assert!(!w.p.status().mic_open);
}

#[tokio::test]
async fn wake_word_is_ignored_when_disabled_and_suspended_when_muted() {
    let mut w = world(false);
    w.mic(&timeline());
    w.p.arm_wake_words(listener()).unwrap();
    w.run_until(9_800).await;
    assert_eq!(w.events(EVENT_WORD_IGNORED).len(), 1, "domyślnie wyłączone");
    assert!(w.events(EVENT_LISTEN_START).is_empty());

    let mut w = world(true);
    w.mic(&timeline());
    w.p.arm_wake_words(listener()).unwrap();
    w.p.input(PipelineInput::SetMuted { muted: true });
    w.run_until(9_800).await;
    let stats = w.p.wake_listener_stats().unwrap();
    assert_eq!(
        stats.scored_frames, 0,
        "wyciszony — model nie dostaje audio"
    );
    assert!(w.events(EVENT_LISTEN_START).is_empty());

    let mut w = world(true);
    w.mic(&timeline());
    w.p.arm_wake_words(listener()).unwrap();
    w.p.input(PipelineInput::SetDoNotDisturb { on: true });
    w.run_until(9_800).await;
    assert!(w.events(EVENT_LISTEN_START).is_empty(), "DND");
    assert!(w.p.wake_listener_stats().unwrap().suspended_frames > 0);
}
