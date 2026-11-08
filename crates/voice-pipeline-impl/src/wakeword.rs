//! Słowa wywoławcze w potoku (`voice-wake` v1, F5): po uzbrojeniu mikrofon jest otwarty stale,
//! ale **przed wykryciem audio trafia wyłącznie do nasłuchu** (`WakeWordListener`: bufor ~2 s,
//! bramka, model KWS) — nie do VAD, STT, pre-rollu ani magistrali. Ramki idą przez DSP (AEC
//! z referencją własnego TTS), żeby agentka nie budziła się własnym głosem.
//!
//! Wykrycie → `WakeInput::WakeWord` → słuchanie z adresatką (zmiana agentki jak przy zwrocie po
//! imieniu) → zwykła ścieżka mikrofonu. Koniec sesji (cisza, PTT, wyciszenie) → z powrotem nasłuch.
//! Wyciszenie i „nie przeszkadzać” wstrzymują nasłuch (bufor czyszczony, audio nieprzyjmowane).

use voice_audio_contract::StreamConfig;
use voice_pipeline_contract::{PipelineError, SwitchSource};
use voice_wake_contract::{WakeInput, WakeWordListener};

use crate::pipeline::Pipeline;

/// Uzbrojony nasłuch.
pub(crate) struct Armed {
    listener: WakeWordListener,
    muted: bool,
    dnd: bool,
}

impl Pipeline {
    /// Uzbraja słowa wywoławcze (wymaga `WakeCfg::wake_words` w automacie aktywacji — inaczej
    /// wykrycia są ignorowane ze zdarzeniem `voice.wake.word_ignored`). Otwiera mikrofon.
    pub fn arm_wake_words(&mut self, listener: WakeWordListener) -> Result<(), PipelineError> {
        if self.mic.is_none() {
            let stream = self
                .audio
                .open_input(None, &StreamConfig::input_default())
                .map_err(|e| PipelineError::Component {
                    component: "audio".into(),
                    reason: format!("mikrofon nie otworzył się: {e}"),
                })?;
            self.mic = Some(stream);
            self.dsp.reset();
        }
        let (muted, dnd) = self
            .wake_words
            .as_ref()
            .map_or((false, false), |a| (a.muted, a.dnd));
        self.wake_words = Some(Armed {
            listener,
            muted,
            dnd,
        });
        self.sync_wake_suspension();
        Ok(())
    }

    /// Rozbraja słowa wywoławcze (mikrofon zamykany, jeśli nie trwa słuchanie).
    pub fn disarm_wake_words(&mut self) {
        if self.wake_words.take().is_some() && !self.wake.is_listening() {
            self.mic = None;
        }
    }

    /// Czy nasłuch jest uzbrojony.
    pub fn wake_words_armed(&self) -> bool {
        self.wake_words.is_some()
    }

    /// Liczniki nasłuchu (Diagnostyka; bez treści).
    pub fn wake_listener_stats(&self) -> Option<voice_wake_contract::ListenerStats> {
        self.wake_words.as_ref().map(|a| a.listener.stats())
    }

    /// Nasłuch działa teraz (uzbrojony i nie trwa słuchanie).
    pub(crate) fn wake_idle(&self) -> bool {
        self.wake_words.is_some() && !self.wake.is_listening()
    }

    /// Wyciszenie / DND z wejść UI.
    pub(crate) fn wake_words_flags(&mut self, muted: Option<bool>, dnd: Option<bool>) {
        if let Some(a) = self.wake_words.as_mut() {
            a.muted = muted.unwrap_or(a.muted);
            a.dnd = dnd.unwrap_or(a.dnd);
        }
        self.sync_wake_suspension();
    }

    /// Wstrzymanie nasłuchu: wyciszenie, DND albo trwające słuchanie.
    pub(crate) fn sync_wake_suspension(&mut self) {
        let listening = self.wake.is_listening();
        if let Some(a) = self.wake_words.as_mut() {
            a.listener.set_suspended(a.muted || a.dnd || listening);
        }
    }

    /// Krok nasłuchu: ramki mikrofonu → DSP → nasłuch; wykrycie otwiera słuchanie.
    pub(crate) fn process_wake_words(&mut self) {
        let now = self.st.now_ms;
        self.wake_input(WakeInput::Tick { now_ms: now });
        if !self.wake_idle() {
            return;
        }
        let Some(mic) = self.mic.as_mut() else {
            return;
        };
        let mut frames = Vec::new();
        while let Some(f) = mic.read() {
            frames.push(f);
        }
        let mut trigger = None;
        for f in frames {
            let processed = match self.dsp.process(&f) {
                Ok(p) => p,
                Err(e) => {
                    self.degraded("dsp", e.to_string());
                    continue;
                }
            };
            let Some(armed) = self.wake_words.as_mut() else {
                return;
            };
            for p in processed {
                match armed.listener.push(&p.frame.pcm) {
                    Ok(Some(t)) => trigger = trigger.or(Some(t)),
                    Ok(None) => {}
                    Err(e) => {
                        self.degraded("wake", e.to_string());
                        return;
                    }
                }
            }
            if trigger.is_some() {
                break;
            }
        }
        if let Some(t) = trigger {
            // Audio frazy (`t.audio`) nie idzie dalej: STT dostaje mowę po wykryciu; nic
            // z poprzedniej sesji (pre-roll, stan VAD) nie przechodzi do nowej.
            self.st.preroll.clear();
            self.vad.reset();
            let persona = t.hit.persona.clone();
            self.wake_input(WakeInput::WakeWord {
                persona: persona.clone(),
                phrase: t.hit.phrase,
                at_ms: now,
            });
            if self.wake.is_listening() {
                self.switch_persona(persona, SwitchSource::Name);
            }
        }
    }
}
