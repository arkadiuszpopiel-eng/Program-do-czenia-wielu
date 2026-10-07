//! Atrapa `voice-tts` (SPEC §Fake): „mowa syntetyczna” (harmoniczne z wysokością presetu) zdanie po
//! zdaniu, deterministyczne znaczniki słów (`Native`), TTFB/RTF sterowane wirtualnie (wpisywane
//! w zdarzenia, bez czekania), symulowane awarie silnika → łańcuch fallback per agentka.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use personas_contract::PersonaId;
use voice_audio_contract::synth::{SpeechParams, synthetic_speech};
use voice_audio_contract::{Frame, MediaTime};
use voice_tts_contract::{
    CancelToken, MarksKind, TTS_RATE, Tts, TtsChunk, TtsError, TtsEvent, TtsHealth, TtsRequest,
    TtsStream, VoiceInfo, VoiceRef, estimate_marks, split_sentences, v0_chains,
};

/// Średni czas na znak przy tempie 1.0 (ms).
pub const MS_PER_CHAR: u32 = 65;

#[derive(Debug, Default)]
struct State {
    events: Vec<TtsEvent>,
    failing: BTreeSet<String>,
    ttfb_ms: u32,
}

/// Atrapa TTS z głosami v0.
#[derive(Debug)]
pub struct FakeTts {
    chains: Vec<(PersonaId, Vec<VoiceRef>)>,
    state: Mutex<State>,
}

impl Default for FakeTts {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeTts {
    /// Atrapa z łańcuchami v0 i TTFB 200 ms.
    pub fn new() -> Self {
        Self {
            chains: v0_chains(),
            state: Mutex::new(State {
                ttfb_ms: 200,
                ..State::default()
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Silnik (`pocket`, `piper`…) zaczyna zawodzić.
    pub fn fail_engine(&self, engine: &str) {
        self.lock().failing.insert(engine.to_owned());
    }

    /// TTFB raportowany w `started` (czas wirtualny).
    pub fn set_ttfb_ms(&self, ms: u32) {
        self.lock().ttfb_ms = ms;
    }

    /// TTFB (ms, czas wirtualny) — uprząż testów stosuje go na zegarze wirtualnym.
    pub fn ttfb_ms(&self) -> u32 {
        self.lock().ttfb_ms
    }

    fn render(voice: &VoiceRef, rate: f32, sentence: &str, seed: u64) -> Vec<f32> {
        let chars = sentence
            .chars()
            .filter(|c| !c.is_whitespace())
            .count()
            .max(1) as u32;
        let ms = (chars * MS_PER_CHAR) as f32 / (voice.preset.rate * rate).max(0.1);
        let params = SpeechParams {
            f0: 200.0 * voice.preset.pitch,
            seed,
            ..SpeechParams::default()
        };
        synthetic_speech(TTS_RATE, ms / 1000.0, params)
    }
}

#[async_trait]
impl Tts for FakeTts {
    fn voices(&self) -> Vec<VoiceInfo> {
        self.chains
            .iter()
            .map(|(p, c)| VoiceInfo {
                persona: p.clone(),
                chain: c.clone(),
            })
            .collect()
    }

    async fn synth(&self, req: TtsRequest, cancel: CancelToken) -> Result<TtsStream, TtsError> {
        if req.text.trim().is_empty() {
            return Err(TtsError::EmptyText);
        }
        let chain = self
            .chains
            .iter()
            .find(|(p, _)| *p == req.persona)
            .map(|(_, c)| c.clone())
            .ok_or_else(|| TtsError::NoVoice(req.persona.to_string()))?;
        let mut st = self.lock();
        let voice = chain
            .iter()
            .find(|v| !st.failing.contains(&v.engine.name()))
            .cloned()
            .ok_or_else(|| TtsError::AllEnginesFailed(req.persona.to_string()))?;
        if voice.engine != chain[0].engine {
            st.events.push(TtsEvent::Fallback {
                persona: req.persona.clone(),
                from: chain[0].engine.name(),
                to: voice.engine.name(),
                reason: "atrapa: awaria silnika".into(),
            });
        }
        let sentences = split_sentences(&req.text);
        let (tx, rx) = tokio::sync::mpsc::channel(sentences.len() + 1);
        if cancel.is_cancelled() {
            return Err(TtsError::Cancelled);
        }
        let (mut offset_ms, mut word_idx) = (0u32, 0u32);
        for (seq, s) in sentences.iter().enumerate() {
            let pcm = Self::render(
                &voice,
                req.style.rate,
                s,
                req.utterance.wrapping_mul(31).wrapping_add(seq as u64),
            );
            let dur = (pcm.len() as u64 * 1000 / u64::from(TTS_RATE)) as u32;
            let words: Vec<&str> = s.split_whitespace().collect();
            let marks = estimate_marks(&words, word_idx, offset_ms, dur);
            if seq == 0 {
                let ttfb_ms = st.ttfb_ms;
                st.events.push(TtsEvent::Started {
                    utterance: req.utterance,
                    persona: req.persona.clone(),
                    engine: voice.engine.name(),
                    ttfb_ms,
                    cached: false,
                });
            }
            st.events.push(TtsEvent::Chunk {
                utterance: req.utterance,
                seq: seq as u32,
                duration_ms: dur,
            });
            let chunk = TtsChunk {
                utterance: req.utterance,
                seq: seq as u32,
                audio: Frame::mono(pcm, TTS_RATE, MediaTime::from_ms(u64::from(offset_ms))),
                marks,
                marks_kind: MarksKind::Native,
                is_last: seq + 1 == sentences.len(),
                engine: voice.engine.name(),
            };
            let _ = tx.try_send(Ok(chunk));
            offset_ms += dur;
            word_idx += words.len() as u32;
        }
        st.events.push(TtsEvent::Finished {
            utterance: req.utterance,
            audio_ms: offset_ms,
        });
        Ok(rx)
    }

    fn stop(&self, utterance: u64) {
        self.lock().events.push(TtsEvent::Stopped { utterance });
    }

    async fn warm(&self, persona: &PersonaId) -> Result<(), TtsError> {
        self.chains
            .iter()
            .any(|(p, _)| p == persona)
            .then_some(())
            .ok_or_else(|| TtsError::NoVoice(persona.to_string()))
    }

    fn health(&self) -> TtsHealth {
        if self.lock().failing.is_empty() {
            TtsHealth::Ready
        } else {
            TtsHealth::Degraded("atrapa: awaria silnika".into())
        }
    }

    fn take_events(&self) -> Vec<TtsEvent> {
        std::mem::take(&mut self.lock().events)
    }
}
