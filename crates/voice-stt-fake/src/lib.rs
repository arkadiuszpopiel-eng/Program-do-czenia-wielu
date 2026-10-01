//! Atrapa `voice-stt` (SPEC §Fake): transkrypty z adnotacji (kolejka tekstów), partiale jako
//! prefiks słów proporcjonalny do audio, deterministyczne znaczniki słów, sterowane opóźnienie
//! (wirtualne — wpisywane w transkrypt), symulacja awarii sidecara GPU (fallback CPU bez utraty
//! wypowiedzi). Wspólne reguły (bramka VAD, prywatność) z kontraktu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use device_profile_contract::Backend;
use voice_audio_contract::Frame;
use voice_stt_contract::{
    Health, Stt, SttCfg, SttError, SttEvent, Transcript, UtteranceAudio, UtteranceId, Word,
    check_privacy,
};

/// Domyślny tekst, gdy skrypt jest pusty.
pub const DEFAULT_TEXT: &str = "zażółć gęślą jaźń";

#[derive(Debug)]
struct State {
    cfg: SttCfg,
    script: VecDeque<String>,
    utterances: HashMap<UtteranceId, UtteranceAudio>,
    events: Vec<SttEvent>,
    latency_ms: u32,
    backend: Backend,
    crash_next: bool,
}

/// Atrapa STT.
#[derive(Debug)]
pub struct FakeStt {
    state: Mutex<State>,
}

impl Default for FakeStt {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeStt {
    /// Nowa atrapa (backend Vulkan, opóźnienie 200 ms).
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                cfg: SttCfg::default(),
                script: VecDeque::new(),
                utterances: HashMap::new(),
                events: Vec::new(),
                latency_ms: 200,
                backend: Backend::Vulkan,
                crash_next: false,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Dopisuje tekst kolejnej wypowiedzi (adnotacja korpusu).
    pub fn script(&self, text: impl Into<String>) {
        self.lock().script.push_back(text.into());
    }

    /// Opóźnienie wpisywane w transkrypty (ms, czas wirtualny).
    pub fn set_latency_ms(&self, ms: u32) {
        self.lock().latency_ms = ms;
    }

    /// Następny final „rozbije” sidecar GPU → fallback CPU (wypowiedź nie ginie).
    pub fn crash_next(&self) {
        self.lock().crash_next = true;
    }

    /// Bieżący backend.
    pub fn backend(&self) -> Backend {
        self.lock().backend
    }
}

fn words_for(text: &str, duration_ms: u32) -> Vec<Word> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let n = words.len().max(1) as u32;
    let step = duration_ms / n;
    words
        .iter()
        .enumerate()
        .map(|(i, w)| Word {
            text: (*w).to_owned(),
            start_ms: i as u32 * step,
            end_ms: (i as u32 + 1) * step,
            confidence: 0.9,
        })
        .collect()
}

fn transcript(
    st: &State,
    id: UtteranceId,
    text: &str,
    duration_ms: u32,
    is_final: bool,
) -> Transcript {
    let words = words_for(text, duration_ms);
    Transcript {
        utterance: id,
        text: text.to_owned(),
        words,
        lang: "pl".into(),
        is_final,
        confidence: 0.9,
        latency_ms: st.latency_ms,
        backend: Some(st.backend),
    }
}

#[async_trait]
impl Stt for FakeStt {
    async fn configure(&self, cfg: SttCfg) -> Result<(), SttError> {
        check_privacy(&cfg)?;
        self.lock().cfg = cfg;
        Ok(())
    }

    async fn start_utterance(&self, id: UtteranceId) -> Result<(), SttError> {
        let mut st = self.lock();
        if st.utterances.contains_key(&id) {
            return Err(SttError::DuplicateUtterance(id));
        }
        st.utterances.insert(id, UtteranceAudio::default());
        Ok(())
    }

    async fn push(&self, id: UtteranceId, frame: &Frame) -> Result<Option<Transcript>, SttError> {
        let mut st = self.lock();
        let two_pass = st.cfg.two_pass;
        let next_text = st
            .script
            .front()
            .cloned()
            .unwrap_or_else(|| DEFAULT_TEXT.to_owned());
        let audio = st
            .utterances
            .get_mut(&id)
            .ok_or(SttError::UnknownUtterance(id))?;
        audio.push(frame)?;
        if !(two_pass.enabled && audio.take_partial_due(two_pass.partial_every_ms)) {
            return Ok(None);
        }
        let duration = audio.duration_ms();
        // Partial = prefiks słów proporcjonalny do audio (≈ 3 słowa na sekundę).
        let n = (duration / 333).max(1) as usize;
        let prefix: Vec<&str> = next_text.split_whitespace().take(n).collect();
        let t = transcript(&st, id, &prefix.join(" "), duration, false);
        st.events.push(SttEvent::Partial {
            transcript: t.clone(),
        });
        Ok(Some(t))
    }

    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError> {
        let mut st = self.lock();
        let audio = st
            .utterances
            .remove(&id)
            .ok_or(SttError::UnknownUtterance(id))?;
        if audio.speech_ms() < st.cfg.min_speech_ms {
            st.events.push(SttEvent::GateRejected {
                utterance: id,
                speech_ms: audio.speech_ms(),
            });
            return Ok(Transcript::empty_final(id));
        }
        if st.crash_next {
            st.crash_next = false;
            let from = st.backend;
            st.backend = Backend::Cpu;
            st.events.push(SttEvent::BackendFallback {
                from,
                to: Backend::Cpu,
                reason: "atrapa: ErrorDeviceLost".into(),
            });
        }
        let text = st
            .script
            .pop_front()
            .unwrap_or_else(|| DEFAULT_TEXT.to_owned());
        let t = transcript(&st, id, &text, audio.duration_ms(), true);
        st.events.push(SttEvent::Final {
            transcript: t.clone(),
        });
        Ok(t)
    }

    async fn cancel(&self, id: UtteranceId) {
        self.lock().utterances.remove(&id);
    }

    fn health(&self) -> Health {
        Health::Ready(self.lock().backend)
    }

    fn take_events(&self) -> Vec<SttEvent> {
        std::mem::take(&mut self.lock().events)
    }
}
