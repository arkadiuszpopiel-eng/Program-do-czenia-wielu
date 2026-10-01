//! [`TtsService`]: chunker → łańcuch silników agentki (fallback per zdanie, z pominięciem chmury
//! w sesji prywatnej) → modyfikacja głosu v0 (wysokość/tempo) → 24 kHz → fragmenty ze znacznikami
//! (natywne przeskalowane o tempo albo estymowane), TTFB, cache fraz, anulowanie.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use async_trait::async_trait;
use personas_contract::PersonaId;
use providers_contract::PrivacyTag;
use tokio::sync::mpsc::Sender;
use voice_audio_contract::{Frame, MediaTime, Resampler};
use voice_tts_contract::{
    CancelToken, MarksKind, TTS_RATE, Tts, TtsChunk, TtsEngine, TtsError, TtsEvent, TtsHealth,
    TtsRequest, TtsStream, VoiceInfo, VoiceRef, WordMark, estimate_marks, split_sentences,
    validate_chains,
};

use crate::cache::{CachedPhrase, PhraseCache};
use crate::engines::{EngineAudio, TtsBackend};
use crate::voice_mod::apply_preset;

#[derive(Default)]
struct Shared {
    events: Vec<TtsEvent>,
    active: HashMap<u64, CancelToken>,
    degraded: Option<String>,
}

type SharedRef = Arc<Mutex<Shared>>;

fn lock(s: &SharedRef) -> MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(|p| p.into_inner())
}

/// Serwis TTS.
pub struct TtsService {
    chains: Vec<(PersonaId, Vec<VoiceRef>)>,
    backends: Arc<HashMap<String, Arc<dyn TtsBackend>>>,
    cache: Option<PhraseCache>,
    shared: SharedRef,
}

impl TtsService {
    /// Serwis z łańcuchami (walidowanymi: odrębność brzmień) i silnikami.
    pub fn new(
        chains: Vec<(PersonaId, Vec<VoiceRef>)>,
        backends: Vec<Arc<dyn TtsBackend>>,
    ) -> Result<Self, TtsError> {
        validate_chains(&chains)?;
        let backends = backends
            .into_iter()
            .map(|b| (b.engine().name(), b))
            .collect();
        Ok(Self {
            chains,
            backends: Arc::new(backends),
            cache: None,
            shared: Arc::default(),
        })
    }

    /// Cache fraz stałych.
    #[must_use]
    pub fn with_cache(mut self, cache: PhraseCache) -> Self {
        self.cache = Some(cache);
        self
    }

    fn chain(&self, persona: &PersonaId) -> Result<Vec<VoiceRef>, TtsError> {
        self.chains
            .iter()
            .find(|(p, _)| p == persona)
            .map(|(_, c)| c.clone())
            .ok_or_else(|| TtsError::NoVoice(persona.to_string()))
    }
}

/// Wynik syntezy zdania jednym ogniwem łańcucha.
struct Sentence {
    pcm: Vec<f32>,
    marks: Vec<WordMark>,
    kind: MarksKind,
    engine: String,
}

async fn synth_sentence(
    backends: &HashMap<String, Arc<dyn TtsBackend>>,
    voice: &VoiceRef,
    req: &TtsRequest,
    text: &str,
    first_idx: u32,
    offset_ms: u32,
) -> Result<Sentence, TtsError> {
    if let TtsEngine::Cloud { .. } = voice.engine {
        if req.privacy == PrivacyTag::Private {
            return Err(TtsError::PrivacyBlocked);
        }
        return Err(TtsError::NotAvailable(
            "chmurowe TTS — adapter w kolejnej fali".into(),
        ));
    }
    let backend = backends.get(&voice.engine.name()).ok_or_else(|| {
        TtsError::NotAvailable(format!(
            "silnik {} nie jest zainstalowany",
            voice.engine.name()
        ))
    })?;
    let EngineAudio { pcm, rate, marks } = backend.synth(text, voice, &req.style).await?;
    let tempo = voice.preset.rate * req.style.rate;
    let modded = apply_preset(&pcm, rate, voice.preset.pitch, tempo);
    let out = Resampler::convert(rate, TTS_RATE, &modded);
    let dur = (out.len() as u64 * 1000 / u64::from(TTS_RATE)) as u32;
    let words: Vec<&str> = text.split_whitespace().collect();
    let (marks, kind) = match marks {
        Some(native) if native.len() == words.len() => {
            let scale = 1.0 / tempo.max(0.1);
            let m = native
                .iter()
                .enumerate()
                .map(|(i, (w, s, e))| WordMark {
                    word_idx: first_idx + i as u32,
                    word: w.clone(),
                    start_ms: offset_ms + (*s as f32 * scale).round() as u32,
                    end_ms: offset_ms + ((*e as f32 * scale).round() as u32).min(dur),
                })
                .collect();
            (m, MarksKind::Native)
        }
        _ => (
            estimate_marks(&words, first_idx, offset_ms, dur),
            MarksKind::Estimated,
        ),
    };
    Ok(Sentence {
        pcm: out,
        marks,
        kind,
        engine: voice.engine.name(),
    })
}

struct Job {
    req: TtsRequest,
    chain: Vec<VoiceRef>,
    backends: Arc<HashMap<String, Arc<dyn TtsBackend>>>,
    cache: Option<(PhraseCache, String)>,
    shared: SharedRef,
    cancel: CancelToken,
    started: Instant,
}

impl Job {
    fn emit(&self, e: TtsEvent) {
        lock(&self.shared).events.push(e);
    }

    async fn run(self, tx: Sender<Result<TtsChunk, TtsError>>) {
        let sentences = split_sentences(&self.req.text);
        let (mut offset_ms, mut word_idx, mut link) = (0u32, 0u32, 0usize);
        let mut whole = CachedPhrase {
            marks: Vec::new(),
            kind: MarksKind::Estimated,
            engine: String::new(),
            pcm: Vec::new(),
        };
        for (seq, text) in sentences.iter().enumerate() {
            if self.cancel.is_cancelled() {
                let _ = tx.send(Err(TtsError::Cancelled)).await;
                return;
            }
            let mut last_err = String::new();
            let mut done = None;
            while link < self.chain.len() {
                match synth_sentence(
                    &self.backends,
                    &self.chain[link],
                    &self.req,
                    text,
                    word_idx,
                    offset_ms,
                )
                .await
                {
                    Ok(s) => {
                        done = Some(s);
                        break;
                    }
                    Err(e) => {
                        last_err = e.to_string();
                        if let Some(next) = self.chain.get(link + 1) {
                            self.emit(TtsEvent::Fallback {
                                persona: self.req.persona.clone(),
                                from: self.chain[link].engine.name(),
                                to: next.engine.name(),
                                reason: last_err.clone(),
                            });
                            lock(&self.shared).degraded =
                                Some(format!("{}: {last_err}", self.chain[link].engine.name()));
                        }
                        link += 1;
                    }
                }
            }
            let Some(s) = done else {
                let _ = tx.send(Err(TtsError::AllEnginesFailed(last_err))).await;
                return;
            };
            let dur = (s.pcm.len() as u64 * 1000 / u64::from(TTS_RATE)) as u32;
            if seq == 0 {
                let ttfb_ms = u32::try_from(self.started.elapsed().as_millis()).unwrap_or(u32::MAX);
                self.emit(TtsEvent::Started {
                    utterance: self.req.utterance,
                    persona: self.req.persona.clone(),
                    engine: s.engine.clone(),
                    ttfb_ms,
                    cached: false,
                });
            }
            self.emit(TtsEvent::Chunk {
                utterance: self.req.utterance,
                seq: seq as u32,
                duration_ms: dur,
            });
            if self.cache.is_some() {
                whole.pcm.extend_from_slice(&s.pcm);
                whole.marks.extend(s.marks.iter().cloned());
                whole.kind = s.kind;
                whole.engine.clone_from(&s.engine);
            }
            let chunk = TtsChunk {
                utterance: self.req.utterance,
                seq: seq as u32,
                audio: Frame::mono(s.pcm, TTS_RATE, MediaTime::from_ms(u64::from(offset_ms))),
                marks: s.marks,
                marks_kind: s.kind,
                is_last: seq + 1 == sentences.len(),
                engine: s.engine,
            };
            if tx.send(Ok(chunk)).await.is_err() {
                return; // odbiorca porzucił strumień
            }
            offset_ms += dur;
            word_idx += text.split_whitespace().count() as u32;
        }
        self.emit(TtsEvent::Finished {
            utterance: self.req.utterance,
            audio_ms: offset_ms,
        });
        if let Some((cache, key)) = &self.cache {
            let _ = cache.put(key, &whole);
        }
        lock(&self.shared).active.remove(&self.req.utterance);
    }
}

#[async_trait]
impl Tts for TtsService {
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
        let chain = self.chain(&req.persona)?;
        if cancel.is_cancelled() {
            return Err(TtsError::Cancelled);
        }
        let started = Instant::now();
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let cache = match (&self.cache, req.cacheable) {
            (Some(c), true) => {
                let key = PhraseCache::key(&req.text, &chain[0].timbre(), req.style.rate);
                if let Some(hit) = c.get(&key) {
                    let ms = (hit.pcm.len() as u64 * 1000 / u64::from(TTS_RATE)) as u32;
                    let mut sh = lock(&self.shared);
                    sh.events.push(TtsEvent::Started {
                        utterance: req.utterance,
                        persona: req.persona.clone(),
                        engine: hit.engine.clone(),
                        ttfb_ms: 0,
                        cached: true,
                    });
                    sh.events.push(TtsEvent::Finished {
                        utterance: req.utterance,
                        audio_ms: ms,
                    });
                    let chunk = TtsChunk {
                        utterance: req.utterance,
                        seq: 0,
                        audio: Frame::mono(hit.pcm, TTS_RATE, MediaTime::ZERO),
                        marks: hit.marks,
                        marks_kind: hit.kind,
                        is_last: true,
                        engine: hit.engine,
                    };
                    let _ = tx.try_send(Ok(chunk));
                    return Ok(rx);
                }
                Some((c.clone(), key))
            }
            _ => None,
        };
        lock(&self.shared)
            .active
            .insert(req.utterance, cancel.clone());
        let job = Job {
            req,
            chain,
            backends: Arc::clone(&self.backends),
            cache,
            shared: Arc::clone(&self.shared),
            cancel,
            started,
        };
        tokio::spawn(job.run(tx));
        Ok(rx)
    }

    fn stop(&self, utterance: u64) {
        let mut sh = lock(&self.shared);
        if let Some(c) = sh.active.remove(&utterance) {
            c.cancel();
        }
        sh.events.push(TtsEvent::Stopped { utterance });
    }

    async fn warm(&self, persona: &PersonaId) -> Result<(), TtsError> {
        let chain = self.chain(persona)?;
        chain
            .iter()
            .any(|v| self.backends.contains_key(&v.engine.name()))
            .then_some(())
            .ok_or_else(|| TtsError::NotAvailable(format!("brak silnika dla {persona}")))
    }

    fn health(&self) -> TtsHealth {
        match &lock(&self.shared).degraded {
            Some(reason) => TtsHealth::Degraded(reason.clone()),
            None if self.backends.is_empty() => TtsHealth::Failed("brak silników".into()),
            None => TtsHealth::Ready,
        }
    }

    fn take_events(&self) -> Vec<TtsEvent> {
        std::mem::take(&mut lock(&self.shared).events)
    }
}
