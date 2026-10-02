//! Opóźnienia składowych w czasie wirtualnym: atrapy podają wartości (`FakeStt::latency_ms`,
//! `FakeTts::ttfb_ms`, TTFT modelu), a te opakowania wstrzymują wynik do chwili, w której zegar
//! `FakeAudio` ją osiągnie. Potok odpytuje przyszłości co krok, więc nie potrzeba budzika.

#![allow(dead_code)]

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures_core::Stream;
use personas_contract::PersonaId;
use providers_contract::CancellationToken;
use voice_audio_contract::{Frame, MediaTime};
use voice_audio_fake::FakeAudio;
use voice_pipeline_contract::{ReplyChunk, ReplyOutcome, ReplyRequest, ReplySource, ReplyStream};
use voice_stt_contract::{Health, Stt, SttCfg, SttError, SttEvent, Transcript, UtteranceId};
use voice_stt_fake::FakeStt;
use voice_tts_contract::{
    CancelToken, Tts, TtsError, TtsEvent, TtsHealth, TtsRequest, TtsStream, VoiceInfo,
};
use voice_tts_fake::FakeTts;

/// Czeka (bez budzika) do chwili `until` zegara wirtualnego.
pub struct VirtualDelay {
    clock: FakeAudio,
    until: MediaTime,
}

impl Future for VirtualDelay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.clock.now() >= self.until {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

pub fn delay(clock: &FakeAudio, ms: u64) -> VirtualDelay {
    VirtualDelay {
        clock: clock.clone(),
        until: clock.now().plus(std::time::Duration::from_millis(ms)),
    }
}

/// STT z opóźnieniem finalu (`FakeStt::latency_ms`) i partiala na żądanie; awaria finalu na żądanie.
pub struct SlowStt {
    pub inner: Arc<FakeStt>,
    pub clock: FakeAudio,
    pub partial_ms: AtomicU64,
    pub fail_next_end: AtomicBool,
}

impl SlowStt {
    pub fn new(inner: Arc<FakeStt>, clock: FakeAudio) -> Self {
        Self {
            inner,
            clock,
            partial_ms: AtomicU64::new(50),
            fail_next_end: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Stt for SlowStt {
    async fn configure(&self, cfg: SttCfg) -> Result<(), SttError> {
        self.inner.configure(cfg).await
    }
    async fn start_utterance(&self, id: UtteranceId) -> Result<(), SttError> {
        self.inner.start_utterance(id).await
    }
    async fn push(&self, id: UtteranceId, frame: &Frame) -> Result<Option<Transcript>, SttError> {
        self.inner.push(id, frame).await
    }
    async fn partial_now(&self, id: UtteranceId) -> Result<Option<Transcript>, SttError> {
        let wait = delay(&self.clock, self.partial_ms.load(Ordering::SeqCst));
        let t = self.inner.partial_now(id).await;
        wait.await;
        t
    }
    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError> {
        let wait = delay(&self.clock, u64::from(self.inner.latency_ms()));
        if self.fail_next_end.swap(false, Ordering::SeqCst) {
            self.inner.cancel(id).await;
            wait.await;
            return Err(SttError::Sidecar(
                "atrapa: sidecar zakończył się (ErrorDeviceLost)".into(),
            ));
        }
        let t = self.inner.end_utterance(id).await;
        wait.await;
        t
    }
    async fn cancel(&self, id: UtteranceId) {
        self.inner.cancel(id).await;
    }
    fn health(&self) -> Health {
        self.inner.health()
    }
    fn take_events(&self) -> Vec<SttEvent> {
        self.inner.take_events()
    }
}

/// TTS z TTFB (`FakeTts::ttfb_ms`) — strumień zwracany po czasie pierwszego fragmentu.
pub struct SlowTts {
    pub inner: Arc<FakeTts>,
    pub clock: FakeAudio,
}

#[async_trait]
impl Tts for SlowTts {
    fn voices(&self) -> Vec<VoiceInfo> {
        self.inner.voices()
    }
    async fn synth(&self, request: TtsRequest, cancel: CancelToken) -> Result<TtsStream, TtsError> {
        let wait = delay(&self.clock, u64::from(self.inner.ttfb_ms()));
        let stream = self.inner.synth(request, cancel).await;
        wait.await;
        stream
    }
    fn stop(&self, utterance: u64) {
        self.inner.stop(utterance);
    }
    async fn warm(&self, persona: &PersonaId) -> Result<(), TtsError> {
        self.inner.warm(persona).await
    }
    fn health(&self) -> TtsHealth {
        self.inner.health()
    }
    fn take_events(&self) -> Vec<TtsEvent> {
        self.inner.take_events()
    }
}

/// Źródło odpowiedzi z TTFT (czas do pierwszego tekstu) w czasie wirtualnym.
pub struct SlowReply {
    pub inner: Arc<dyn ReplySource>,
    pub clock: FakeAudio,
    pub ttft_ms: AtomicU64,
    /// Żądania (pochodzenie tury głosowej) i wyniki weryfikacji mówcy — do asercji testów.
    pub requests: std::sync::Mutex<Vec<ReplyRequest>>,
    pub checks: std::sync::Mutex<Vec<(u64, voice_speaker_contract::SpeakerCheck)>>,
}

struct DelayedStream {
    inner: ReplyStream,
    gate: VirtualDelay,
}

impl Stream for DelayedStream {
    type Item = ReplyChunk;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<ReplyChunk>> {
        if Pin::new(&mut self.gate).poll(cx).is_pending() {
            return Poll::Pending;
        }
        self.inner.as_mut().poll_next(cx)
    }
}

impl ReplySource for SlowReply {
    fn start(&self, request: ReplyRequest, cancel: CancellationToken) -> ReplyStream {
        self.requests.lock().unwrap().push(request.clone());
        let gate = delay(&self.clock, self.ttft_ms.load(Ordering::SeqCst));
        Box::pin(DelayedStream {
            inner: self.inner.start(request, cancel),
            gate,
        })
    }
    fn finish(&self, turn: u64, outcome: ReplyOutcome) {
        self.inner.finish(turn, outcome);
    }
    fn speaker_checked(&self, turn: u64, check: voice_speaker_contract::SpeakerCheck) {
        self.checks.lock().unwrap().push((turn, check));
    }
}
