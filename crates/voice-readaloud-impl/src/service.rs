//! Usługa czytania: tekst ze źródła → automat (zdania, sterowanie) → `voice-tts` głosem agentki
//! (`PrivacyTag::Private` — treść okna nie idzie do chmurowego TTS, chyba że konfiguracja na to
//! pozwala) → wyjście audio; głośnik jako zasób wyłączny `scheduler-lite` (`Holder::Persona`).
//! Koniec zdania = `PlaybackFinished` z wyjścia; stop/pauza: anulowanie syntezy i `stop_all`.

use std::sync::Arc;

use async_trait::async_trait;
use personas_contract::PersonaId;
use providers_contract::PrivacyTag;
use scheduler_lite_contract::{Holder, Lease, Priority, Resource, SchedulerLite};
use voice_audio_contract::{AudioEvent, OutputStream, SourceId};
use voice_readaloud_contract::{
    ReadAloud, ReadAloudCfg, ReadAloudError, ReadAloudEvent, ReadAloudMachine, ReadCommand,
    ReadControl, ReadPhase, ReadScope, ReadStatus, ShareConsent, TextSource,
};
use voice_tts_contract::{CancelToken, SpeechStyle, Tts, TtsRequest, TtsStream};
use voice_wake_contract::lease_now;

/// Pierwszy identyfikator wypowiedzi czytania (osobna przestrzeń od potoku rozmowy).
pub const UTTERANCE_BASE: u64 = 1 << 61;

struct Playing {
    seq: u64,
    utterance: u64,
    stream: Option<TtsStream>,
    cancel: CancelToken,
}

/// Składniki usługi.
pub struct ReadAloudParts {
    /// Źródło tekstu (okno na pierwszym planie).
    pub source: Arc<dyn TextSource>,
    /// Synteza.
    pub tts: Arc<dyn Tts>,
    /// Wyjście audio (mikser).
    pub output: Box<dyn OutputStream>,
    /// Głośnik jako zasób wyłączny.
    pub scheduler: Arc<dyn SchedulerLite>,
}

/// Usługa czytania na głos.
pub struct ReadAloudService {
    parts: ReadAloudParts,
    cfg: ReadAloudCfg,
    machine: ReadAloudMachine,
    persona: PersonaId,
    playing: Option<Playing>,
    lease: Option<Lease>,
    next_utterance: u64,
}

impl ReadAloudService {
    /// Usługa.
    pub fn new(parts: ReadAloudParts, cfg: ReadAloudCfg) -> Self {
        Self {
            parts,
            cfg,
            machine: ReadAloudMachine::new(cfg),
            persona: PersonaId::alfa(),
            playing: None,
            lease: None,
            next_utterance: UTTERANCE_BASE,
        }
    }

    async fn execute(&mut self, commands: Vec<ReadCommand>) {
        for c in commands {
            match c {
                ReadCommand::Stop { seq } => self.stop_playing(seq),
                ReadCommand::Speak {
                    seq, text, rate, ..
                } => self.speak(seq, text, rate).await,
            }
        }
        if matches!(
            self.machine.status().phase,
            ReadPhase::Idle | ReadPhase::Finished
        ) {
            self.lease = None;
        }
    }

    fn stop_playing(&mut self, seq: u64) {
        if let Some(p) = self.playing.take_if(|p| p.seq == seq) {
            p.cancel.cancel();
            self.parts.tts.stop(p.utterance);
            let _ = self.parts.output.stop_all();
        }
    }

    async fn speak(&mut self, seq: u64, text: String, rate: f32) {
        self.next_utterance += 1;
        let utterance = self.next_utterance;
        let cancel = CancelToken::new();
        let request = TtsRequest {
            utterance,
            persona: self.persona.clone(),
            text,
            style: SpeechStyle {
                rate,
                ..SpeechStyle::default()
            },
            cacheable: false,
            privacy: if self.cfg.allow_cloud_tts {
                PrivacyTag::Normal
            } else {
                PrivacyTag::Private
            },
        };
        match self.parts.tts.synth(request, cancel.clone()).await {
            Ok(stream) => {
                self.playing = Some(Playing {
                    seq,
                    utterance,
                    stream: Some(stream),
                    cancel,
                });
            }
            Err(e) => self.machine.failed(seq, &format!("synteza: {e}")),
        }
    }

    /// Przenosi gotowe fragmenty syntezy do wyjścia; zwraca błąd syntezy, jeśli był.
    fn pump_audio(&mut self) -> Option<(u64, String)> {
        let p = self.playing.as_mut()?;
        let stream = p.stream.as_mut()?;
        let source = SourceId::Tts(self.persona.clone());
        loop {
            match stream.try_recv() {
                Ok(Ok(chunk)) => {
                    if let Err(e) = self.parts.output.play(&source, p.utterance, &chunk.audio) {
                        return Some((p.seq, format!("głośnik: {e}")));
                    }
                    if chunk.is_last {
                        let _ = self.parts.output.end_utterance(p.utterance);
                        p.stream = None;
                        return None;
                    }
                }
                Ok(Err(e)) => return Some((p.seq, format!("synteza: {e}"))),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => return None,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    let _ = self.parts.output.end_utterance(p.utterance);
                    p.stream = None;
                    return None;
                }
            }
        }
    }
}

#[async_trait]
impl ReadAloud for ReadAloudService {
    async fn start(
        &mut self,
        scope: ReadScope,
        persona: PersonaId,
    ) -> Result<ReadStatus, ReadAloudError> {
        let text = match self.parts.source.read(scope, self.cfg.max_chars) {
            Ok(t) => t,
            Err(e) => {
                if let ReadAloudError::Refused(reason) = &e {
                    self.machine.refuse(*reason);
                }
                return Err(e);
            }
        };
        let lease = lease_now(
            self.parts.scheduler.as_ref(),
            Resource::Speaker,
            Holder::Persona(persona.clone()),
            Priority::Interactive,
        )
        .map_err(|e| ReadAloudError::Platform(format!("głośnik zajęty: {e}")))?;
        self.persona = persona;
        self.lease = Some(lease);
        let commands = self.machine.load(text);
        self.execute(commands).await;
        Ok(self.machine.status())
    }

    async fn step(&mut self) -> ReadStatus {
        if self.lease.as_ref().is_some_and(Lease::is_revoked) {
            // Kill-switch / mowa użytkownika odebrała głośnik.
            let commands = self.machine.control(ReadControl::Stop);
            self.execute(commands).await;
            return self.machine.status();
        }
        if let Some((seq, reason)) = self.pump_audio() {
            self.stop_playing(seq);
            self.machine.failed(seq, &reason);
        }
        let mut commands = Vec::new();
        for e in self.parts.output.poll_events() {
            if let AudioEvent::PlaybackFinished {
                utterance,
                stopped: false,
                ..
            } = e
                && let Some(p) = self.playing.take_if(|p| p.utterance == utterance)
            {
                commands.extend(self.machine.finished(p.seq));
            }
        }
        self.execute(commands).await;
        self.machine.status()
    }

    async fn control(&mut self, control: ReadControl) -> ReadStatus {
        let commands = self.machine.control(control);
        self.execute(commands).await;
        self.machine.status()
    }

    fn status(&self) -> ReadStatus {
        self.machine.status()
    }

    fn share_with_model(&self, consent: Option<&ShareConsent>) -> Result<String, ReadAloudError> {
        let source = self.machine.source().ok_or(ReadAloudError::NotReading)?;
        source
            .text
            .for_model(consent, &source.app)
            .ok_or(ReadAloudError::ConsentRequired)
    }

    fn take_events(&mut self) -> Vec<ReadAloudEvent> {
        self.machine.take_events()
    }
}
