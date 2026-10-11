//! Rozmowa „na sucho” na prawdziwych silnikach tej maszyny (pomijana bez `ALFA_LIVE_TALK=1`):
//! zamiast mikrofonu nagranie (`ALFA_LIVE_TALK_WAV`, zdanie po polsku), a dalej prawdziwy tor
//! słuchania — tłumienie szumu, wykrywanie mowy Silero, koniec wypowiedzi (Smart Turn), rozpoznawanie
//! (Qwen3-ASR ze strażnikiem i Whisperem w zapasie) — aż tekst trafi do czatu, oraz prawdziwa
//! synteza odpowiedzi. Pliki silników i modeli bierze z katalogu danych programu (`%LOCALAPPDATA%`).
//!
//! `ALFA_LIVE_TALK_EXPECT` — słowa (po przecinku), które muszą być w rozpoznanym tekście.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use app_api::AppPaths;
use app_api::dto::{SttEngineKind, TurnEngineKind};
use app_api::events::EventHub;
use app_api::ports::{VoicePort, VoiceUnavailable};
use app_voice::{PipelineVoice, SystemVoice};
use common::TestChat;
use scheduler_lite_fake::FakeScheduler;
use voice_audio_fake::FakeAudio;
use voice_pipeline_contract::PipelineCfg;

const LEAD_MS: usize = 1_000;
const GAP_MS: usize = 9_000;
const TAIL_MS: usize = 6_000;

fn wav_16k(path: &str) -> Vec<f32> {
    let bytes = std::fs::read(path).unwrap();
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .map(|i| i + 8)
        .unwrap();
    bytes[data..]
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_engines_hear_a_sentence_and_answer() {
    if std::env::var("ALFA_LIVE_TALK").is_err() {
        return;
    }
    let wav = std::env::var("ALFA_LIVE_TALK_WAV").expect("ALFA_LIVE_TALK_WAV");
    let mut log = std::io::stderr();
    let paths = AppPaths::from_env().unwrap();
    let audio = FakeAudio::new();
    let speech = wav_16k(&wav);
    // Dwie wypowiedzi: pierwsza trafia na zimne silniki, druga pokazuje zwykłe tempo rozmowy.
    let mut mic = vec![0.0f32; LEAD_MS * 16];
    mic.extend_from_slice(&speech);
    mic.extend(std::iter::repeat_n(0.0f32, GAP_MS * 16));
    mic.extend_from_slice(&speech);
    mic.extend(std::iter::repeat_n(0.0f32, TAIL_MS * 16));
    audio.set_mic_signal(&mic, 16_000, false);
    let first_end = u64::try_from(LEAD_MS + speech.len() / 16).unwrap();
    let second_end = first_end + u64::try_from(GAP_MS + speech.len() / 16).unwrap();

    let factory = Arc::new(
        SystemVoice::new(
            paths.clone(),
            Arc::new(audio.clone()),
            Some(app_modules::tts::engines(&paths)),
            Arc::new(FakeScheduler::new()),
            None,
        )
        .with_clock(Arc::new(audio.clone())),
    );
    let events = EventHub::start(Duration::from_millis(5));
    let mut rx = events.subscribe();
    let t0 = Instant::now();
    tokio::spawn(async move {
        let mut last = String::new();
        while let Ok(batch) = rx.recv().await {
            for e in batch.iter() {
                if let app_api::dto::AlfaEvent::VoicePill { state } = e {
                    let line = format!("{:?} {:?} {:?}", state.mic, state.speaker, state.partial);
                    if line != last {
                        let _ = writeln!(
                            std::io::stderr(),
                            "[{:>6} ms] {line}",
                            t0.elapsed().as_millis()
                        );
                        last = line;
                    }
                }
            }
        }
    });
    let voice = PipelineVoice::new(
        Arc::new(VoiceUnavailable),
        Some(factory),
        events,
        None,
        PipelineCfg::default(),
    );
    let chat = Arc::new(TestChat::default());
    voice.attach(chat.clone());

    let before = voice.features().await.engines;
    writeln!(log, "silniki przed rozmową: {before:?}").unwrap();
    assert_eq!(before.stt.engine, SttEngineKind::QwenAsr, "model Qwen3-ASR");
    assert_eq!(before.turn.engine, TurnEngineKind::SmartTurn, "Smart Turn");

    // Zegar atrapy mikrofonu idzie w tempie rzeczywistym — silniki liczą naprawdę.
    let clock = audio.clone();
    let ticker = tokio::spawn(async move {
        let mut every = tokio::time::interval(Duration::from_millis(10));
        loop {
            every.tick().await;
            clock.advance(Duration::from_millis(10));
        }
    });
    let started = Instant::now();
    voice.set_mic_enabled(true).await.unwrap();

    for (n, speech_end_ms) in [(1usize, first_end), (2, second_end)] {
        let deadline = Instant::now() + Duration::from_secs(60);
        while chat.turns.lock().unwrap().len() < n {
            assert!(
                Instant::now() < deadline,
                "czat nie dostał wypowiedzi {n} w 60 s; stan: {:?}",
                voice.status().await
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let heard_at_ms = audio.now().as_ms();
        let turn = chat.turns.lock().unwrap()[n - 1].clone();
        writeln!(
            log,
            "wypowiedź {n}: „{}” ({}); od końca mowy do czatu: {} ms (od startu {} ms)",
            turn.text,
            turn.persona,
            heard_at_ms.saturating_sub(speech_end_ms),
            started.elapsed().as_millis()
        )
        .unwrap();
        let text = turn.text.to_lowercase();
        if let Ok(expect) = std::env::var("ALFA_LIVE_TALK_EXPECT") {
            for word in expect.split(',').map(str::trim).filter(|w| !w.is_empty()) {
                assert!(
                    text.contains(&word.to_lowercase()),
                    "brak „{word}” w: {text}"
                );
            }
        }
        // Odpowiedź czatu ma zostać powiedziana prawdziwym głosem (dźwięk na wyjściu atrapy).
        let spoken_deadline = Instant::now() + Duration::from_secs(30);
        let from = usize::try_from(heard_at_ms).unwrap() * 48;
        loop {
            let recorded = audio.recorded_range(from, usize::MAX);
            if recorded.iter().any(|s| s.abs() > 0.02) {
                writeln!(
                    log,
                    "wypowiedź {n}: odpowiedź słyszalna po {} ms od przekazania tekstu",
                    audio.now().as_ms().saturating_sub(heard_at_ms)
                )
                .unwrap();
                break;
            }
            assert!(
                Instant::now() < spoken_deadline,
                "odpowiedź nie została wypowiedziana"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let after = voice.features().await.engines;
    writeln!(log, "silniki po rozmowie: {after:?}").unwrap();
    let guard = after.stt.guard.expect("stan strażnika po rozmowie");
    assert!(guard.samples >= 2, "{guard:?}");
    // Koniec wypowiedzi naprawdę oceniał model słuchający dźwięku (a nie zapasowa ocena z tekstu).
    let turn = after.turn.stats.expect("liczniki Smart Turn po rozmowie");
    writeln!(log, "Smart Turn: {turn:?}").unwrap();
    assert!(turn.scored >= 2 && turn.failed == 0, "{turn:?}");
    voice.set_mic_enabled(false).await.unwrap();
    ticker.abort();
}
