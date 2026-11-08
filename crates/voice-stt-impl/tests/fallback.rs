//! [`GuardedStt`] na udawanych silnikach: odrzut ciszy, przejście na zapas po awarii i po fladze,
//! bezpiecznik po trzech awariach, lista zmyśleń, telemetria.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use voice_audio_contract::{AudioFormat, Frame, MediaTime};
use voice_stt_contract::{
    Health, LangMode, Stt, SttCfg, SttError, SttEvent, Transcript, UtteranceId,
};
use voice_stt_impl::breaker::BreakerState;
use voice_stt_impl::fallback::GuardedStt;

/// Silnik ze skryptem: zwraca zadany tekst i język albo błąd; liczy wywołania.
#[derive(Default)]
struct Scripted {
    text: Mutex<String>,
    lang: Mutex<String>,
    fail: AtomicBool,
    ends: AtomicUsize,
    starts: AtomicUsize,
    pushed: AtomicUsize,
}

impl Scripted {
    fn says(text: &str, lang: &str) -> Arc<Self> {
        let s = Self::default();
        *s.text.lock().unwrap() = text.into();
        *s.lang.lock().unwrap() = lang.into();
        Arc::new(s)
    }
}

#[async_trait]
impl Stt for Scripted {
    async fn configure(&self, _cfg: SttCfg) -> Result<(), SttError> {
        Ok(())
    }
    async fn start_utterance(&self, _id: UtteranceId) -> Result<(), SttError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn push(&self, _id: UtteranceId, _f: &Frame) -> Result<Option<Transcript>, SttError> {
        self.pushed.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError> {
        self.ends.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(SttError::Sidecar("awaria".into()));
        }
        Ok(Transcript {
            text: self.text.lock().unwrap().clone(),
            lang: self.lang.lock().unwrap().clone(),
            ..Transcript::empty_final(id)
        })
    }
    async fn cancel(&self, _id: UtteranceId) {}
    fn health(&self) -> Health {
        Health::Stopped
    }
    fn take_events(&self) -> Vec<SttEvent> {
        Vec::new()
    }
}

/// Ramki 20 ms: mowa (ton 0,2) albo cisza.
fn frames(ms: usize, amp: f32) -> Vec<Frame> {
    (0..ms / 20)
        .map(|n| {
            let pcm: Vec<f32> = (0..320)
                .map(|i| ((n * 320 + i) as f32 * 0.2).sin() * amp)
                .collect();
            Frame::new(pcm, AudioFormat::mono(16_000), MediaTime::ZERO).unwrap()
        })
        .collect()
}

async fn say(stt: &GuardedStt, id: u64, audio: &[Frame]) -> Result<Transcript, SttError> {
    let id = UtteranceId(id);
    stt.start_utterance(id).await?;
    for f in audio {
        stt.push(id, f).await?;
    }
    stt.end_utterance(id).await
}

#[tokio::test]
async fn clean_result_comes_from_the_primary_engine() {
    let (main, spare) = (
        Scripted::says("Dzień dobry.", "pl"),
        Scripted::says("zapas", "pl"),
    );
    let stt = GuardedStt::new(main.clone(), spare.clone());
    let t = say(&stt, 1, &frames(1000, 0.2)).await.unwrap();
    assert_eq!(t.text, "Dzień dobry.");
    assert_eq!(spare.ends.load(Ordering::SeqCst), 0);
    let status = stt.status();
    assert_eq!(status.counts, (1, 0, 0));
    assert!(status.processing_ms.is_some());
}

#[tokio::test]
async fn silence_is_rejected_before_any_engine_runs() {
    let (main, spare) = (
        Scripted::says("zmyślone", "pl"),
        Scripted::says("zapas", "pl"),
    );
    let stt = GuardedStt::new(main.clone(), spare.clone());
    let t = say(&stt, 1, &frames(1000, 0.0005)).await.unwrap();
    assert!(t.text.is_empty() && t.is_final);
    assert_eq!(
        main.ends.load(Ordering::SeqCst) + spare.ends.load(Ordering::SeqCst),
        0
    );
    assert!(
        stt.take_events()
            .iter()
            .any(|e| matches!(e, SttEvent::GateRejected { .. }))
    );
    assert_eq!(
        stt.status().processing_ms,
        None,
        "odrzuty nie liczą się do czasów"
    );
}

#[tokio::test]
async fn failure_replays_the_same_audio_on_the_fallback() {
    let (main, spare) = (
        Scripted::says("", "pl"),
        Scripted::says("Tekst z zapasu.", "pl"),
    );
    main.fail.store(true, Ordering::SeqCst);
    let stt = GuardedStt::new(main.clone(), spare.clone());
    let audio = frames(1000, 0.2);
    let t = say(&stt, 1, &audio).await.unwrap();
    assert_eq!(t.text, "Tekst z zapasu.");
    assert_eq!(
        spare.pushed.load(Ordering::SeqCst),
        audio.len(),
        "całe nagranie powtórzone"
    );
    assert_eq!(stt.status().counts, (1, 1, 0));
}

#[tokio::test]
async fn flagged_result_is_replaced_and_foreign_language_is_blanked() {
    // Zapętlony dekoder → wynik z zapasu.
    let (main, spare) = (
        Scripted::says("tak tak tak tak tak", "pl"),
        Scripted::says("Tak.", "pl"),
    );
    let stt = GuardedStt::new(main.clone(), spare.clone());
    assert_eq!(say(&stt, 1, &frames(1000, 0.2)).await.unwrap().text, "Tak.");
    // Chiński przy żądanym polskim i pusty zapas → tekst skasowany.
    let (main, spare) = (Scripted::says("你好", "zh"), Scripted::says("", "pl"));
    let stt = GuardedStt::new(main, spare);
    let cfg = SttCfg {
        language: LangMode::Pl,
        ..SttCfg::default()
    };
    stt.configure(cfg).await.unwrap();
    assert!(
        say(&stt, 2, &frames(1000, 0.2))
            .await
            .unwrap()
            .text
            .is_empty()
    );
    assert_eq!(stt.status().counts, (1, 0, 1), "flaga bez zastępstwa");
}

#[tokio::test]
async fn hallucinated_phrase_becomes_empty_text() {
    let (main, spare) = (
        Scripted::says("Dziękuję za oglądanie!", "pl"),
        Scripted::says("x", "pl"),
    );
    let stt = GuardedStt::new(main, spare);
    assert!(
        say(&stt, 1, &frames(1000, 0.2))
            .await
            .unwrap()
            .text
            .is_empty()
    );
}

#[tokio::test]
async fn breaker_opens_after_three_failures_and_can_be_reset() {
    let (main, spare) = (
        Scripted::says("główny", "pl"),
        Scripted::says("zapas", "pl"),
    );
    main.fail.store(true, Ordering::SeqCst);
    let stt = GuardedStt::new(main.clone(), spare.clone());
    for id in 1..=3 {
        assert_eq!(
            say(&stt, id, &frames(600, 0.2)).await.unwrap().text,
            "zapas"
        );
    }
    let status = stt.status();
    assert_eq!(
        (status.breaker, status.failures, status.opened_count),
        (BreakerState::Open, 3, 1)
    );
    assert!(status.retry_in_s.is_some());
    assert!(matches!(stt.health(), Health::Degraded(_)));
    // Bezpiecznik otwarty: silnik główny nie jest nawet uruchamiany.
    let before = main.starts.load(Ordering::SeqCst);
    assert_eq!(say(&stt, 4, &frames(600, 0.2)).await.unwrap().text, "zapas");
    assert_eq!(main.starts.load(Ordering::SeqCst), before);
    // Ręczne zamknięcie i naprawiony silnik → wraca główny.
    main.fail.store(false, Ordering::SeqCst);
    stt.reset_breaker();
    assert_eq!(
        say(&stt, 5, &frames(600, 0.2)).await.unwrap().text,
        "główny"
    );
    assert_eq!(stt.status().breaker, BreakerState::Closed);
}

#[tokio::test]
async fn unknown_and_duplicate_utterances_are_errors() {
    let stt = GuardedStt::new(Scripted::says("a", "pl"), Scripted::says("b", "pl"));
    assert!(matches!(
        stt.end_utterance(UtteranceId(9)).await,
        Err(SttError::UnknownUtterance(_))
    ));
    stt.start_utterance(UtteranceId(1)).await.unwrap();
    assert!(matches!(
        stt.start_utterance(UtteranceId(1)).await,
        Err(SttError::DuplicateUtterance(_))
    ));
    stt.cancel(UtteranceId(1)).await;
    assert!(stt.start_utterance(UtteranceId(1)).await.is_ok());
}
