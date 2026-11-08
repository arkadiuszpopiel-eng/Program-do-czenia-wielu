//! Ścieżka audio dyktowania: mikrofon (dzierżawa `scheduler-lite`, `Holder::User`) → resampling
//! do 16 kHz → wypowiedzi STT → final → [`Dictation::on_final`].
//!
//! - Push-to-talk: jedna wypowiedź na czas przytrzymania (`begin` … `end`).
//! - Przełącznik: wypowiedzi wyznacza VAD (pre-roll 300 ms), sesja do „koniec dyktowania”,
//!   ponownego przełączenia albo bezczynności (`DictationCfg::idle_stop_ms`).
//! - Audio żyje tylko w STT bieżącej wypowiedzi; mikrofon jest zamykany razem z sesją.

use std::collections::VecDeque;
use std::sync::Arc;

use scheduler_lite_contract::{Holder, Lease, Priority, Resource, SchedulerLite};
use voice_audio_contract::{
    AudioIo, Frame, InputStream, MediaTime, PIPELINE_RATE, Resampler, StreamConfig,
};
use voice_dictation_contract::{
    Dictation, DictationError, DictationMode, DictationPhase, DictationStatus,
};
use voice_stt_contract::{Stt, UtteranceId};
use voice_vad_contract::{Vad, VadEvent};
use voice_wake_contract::lease_now;

/// Pre-roll przed początkiem mowy w trybie przełącznika (ramki 10 ms).
const PREROLL_FRAMES: usize = 30;

/// Składniki audio.
pub struct DictationAudio {
    /// Urządzenia.
    pub audio: Arc<dyn AudioIo>,
    /// Wykrywanie mowy (tryb przełącznika).
    pub vad: Box<dyn Vad>,
    /// Rozpoznawanie mowy.
    pub stt: Arc<dyn Stt>,
    /// Mikrofon jako zasób wyłączny.
    pub scheduler: Arc<dyn SchedulerLite>,
}

/// Runner: audio → STT → dyktowanie.
pub struct DictationRunner<D: Dictation> {
    parts: DictationAudio,
    dictation: D,
    mic: Option<Box<dyn InputStream>>,
    lease: Option<Lease>,
    resampler: Option<Resampler>,
    mode: Option<DictationMode>,
    utt: Option<UtteranceId>,
    next_utt: u64,
    preroll: VecDeque<Frame>,
    samples: u64,
}

fn platform(e: impl std::fmt::Display) -> DictationError {
    DictationError::Platform(e.to_string())
}

impl<D: Dictation> DictationRunner<D> {
    /// Runner na składnikach i usłudze dyktowania.
    pub fn new(parts: DictationAudio, dictation: D) -> Self {
        Self {
            parts,
            dictation,
            mic: None,
            lease: None,
            resampler: None,
            mode: None,
            utt: None,
            next_utt: 0,
            preroll: VecDeque::new(),
            samples: 0,
        }
    }

    /// Usługa dyktowania.
    pub fn dictation(&mut self) -> &mut D {
        &mut self.dictation
    }

    /// Czy mikrofon jest otwarty.
    pub fn mic_open(&self) -> bool {
        self.mic.is_some()
    }

    async fn open_utterance(&mut self) -> Result<(), DictationError> {
        self.next_utt += 1;
        let id = UtteranceId(self.next_utt);
        self.parts.stt.start_utterance(id).await.map_err(platform)?;
        self.utt = Some(id);
        Ok(())
    }

    /// Start sesji (PTT: wciśnięcie; przełącznik: włączenie). Cel = okno na pierwszym planie.
    pub async fn begin(
        &mut self,
        mode: DictationMode,
        now_ms: u64,
    ) -> Result<DictationStatus, DictationError> {
        let status = self.dictation.start(mode, now_ms)?;
        let lease = lease_now(
            self.parts.scheduler.as_ref(),
            Resource::Mic,
            Holder::User,
            Priority::UserSpeech,
        );
        let lease = match lease {
            Ok(l) => l,
            Err(e) => {
                self.dictation.stop();
                return Err(platform(format!("mikrofon zajęty: {e}")));
            }
        };
        let stream = match self
            .parts
            .audio
            .open_input(None, &StreamConfig::input_default())
        {
            Ok(s) => s,
            Err(e) => {
                self.dictation.stop();
                return Err(platform(format!("mikrofon nie otworzył się: {e}")));
            }
        };
        self.resampler = Some(Resampler::new(stream.format().sample_rate, PIPELINE_RATE));
        self.mic = Some(stream);
        self.lease = Some(lease);
        self.mode = Some(mode);
        self.parts.vad.reset();
        self.preroll.clear();
        if mode == DictationMode::PushToTalk {
            self.open_utterance().await?;
        }
        Ok(status)
    }

    async fn finish_utterance(&mut self, now_ms: u64) -> Result<(), DictationError> {
        let Some(id) = self.utt.take() else {
            return Ok(());
        };
        let t = self.parts.stt.end_utterance(id).await.map_err(platform)?;
        if !t.text.trim().is_empty() {
            self.dictation.on_final(&t.text, now_ms)?;
        }
        Ok(())
    }

    /// Koniec sesji (PTT: puszczenie — wypowiedź jest domykana i wpisywana; przełącznik: wyłączenie).
    pub async fn end(&mut self, now_ms: u64) -> Result<DictationStatus, DictationError> {
        self.drain_mic().await?;
        let result = self.finish_utterance(now_ms).await;
        self.dictation.tick(now_ms);
        let status = self.dictation.stop();
        self.close().await;
        result.map(|()| status)
    }

    async fn close(&mut self) {
        if let Some(id) = self.utt.take() {
            // Porzucenie wypowiedzi (audio nie jest dalej przetwarzane).
            self.parts.stt.cancel(id).await;
        }
        self.mic = None;
        self.lease = None;
        self.resampler = None;
        self.mode = None;
        self.preroll.clear();
    }

    async fn drain_mic(&mut self) -> Result<(), DictationError> {
        let Some(mic) = self.mic.as_mut() else {
            return Ok(());
        };
        let mut raw = Vec::new();
        while let Some(f) = mic.read() {
            raw.extend(f.to_mono());
        }
        let mut pcm = Vec::new();
        if let Some(r) = self.resampler.as_mut() {
            r.process(&raw, &mut pcm);
        }
        for chunk in pcm.chunks(160) {
            let ts = MediaTime::from_samples(self.samples, PIPELINE_RATE);
            self.samples += chunk.len() as u64;
            self.frame(Frame::mono(chunk.to_vec(), PIPELINE_RATE, ts))
                .await?;
        }
        Ok(())
    }

    async fn frame(&mut self, f: Frame) -> Result<(), DictationError> {
        if self.mode == Some(DictationMode::Toggle) {
            let events = self.parts.vad.push(&f).map_err(platform)?;
            for e in events {
                match e {
                    VadEvent::SpeechStart { .. } if self.utt.is_none() => {
                        self.open_utterance().await?;
                        if let Some(id) = self.utt {
                            for p in std::mem::take(&mut self.preroll) {
                                self.parts.stt.push(id, &p).await.map_err(platform)?;
                            }
                        }
                    }
                    VadEvent::SpeechEnd { .. } => {
                        let now = f.ts.as_ms();
                        if let Some(id) = self.utt {
                            self.parts.stt.push(id, &f).await.map_err(platform)?;
                        }
                        self.finish_utterance(now).await?;
                        return Ok(());
                    }
                    VadEvent::SpeechStart { .. } => {}
                }
            }
        }
        match self.utt {
            Some(id) => {
                self.parts.stt.push(id, &f).await.map_err(platform)?;
            }
            None => {
                self.preroll.push_back(f);
                while self.preroll.len() > PREROLL_FRAMES {
                    self.preroll.pop_front();
                }
            }
        }
        Ok(())
    }

    /// Krok (~10–20 ms): audio → STT → wpisywanie, obserwacja okna, ponowienia, bezczynność.
    pub async fn step(&mut self, now_ms: u64) -> Result<DictationStatus, DictationError> {
        if self.lease.as_ref().is_some_and(Lease::is_revoked) {
            // Kill-switch / scheduler odebrał mikrofon.
            self.dictation.stop();
            self.close().await;
            return Ok(self.dictation.status());
        }
        self.drain_mic().await?;
        let status = self.dictation.tick(now_ms);
        if status.phase == DictationPhase::Idle && self.mic.is_some() {
            self.close().await;
        }
        Ok(status)
    }
}
