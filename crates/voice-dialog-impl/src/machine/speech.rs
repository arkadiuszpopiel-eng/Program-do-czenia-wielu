//! Mowa agentki: głośnik (zasób wyłączny), fragmenty TTS, znaczniki, postęp, koniec,
//! mowa proaktywna i wznawianie od punktu cięcia.

use voice_dialog_contract::{
    Command, DialogNotice, DialogPhase, InterruptClassifier, MarkSource, PendingSpeech,
    ProactiveLabel, ProactiveMode, ProactiveRejection, SpokenChunk, UserTurn, Utterance,
    UtteranceId, WordMark,
};
use voice_persona_contract::PersonaId;

use super::Step;

impl<C: InterruptClassifier> Step<'_, C> {
    fn alloc(&mut self) -> UtteranceId {
        self.s.next_utterance += 1;
        UtteranceId(self.s.next_utterance)
    }

    /// Zwalnia głośnik trzymany przez automat.
    pub(crate) fn release_speaker(&mut self) {
        if let Some(id) = self.s.speaker_held.take() {
            let persona = self
                .s
                .utterance
                .as_ref()
                .filter(|u| u.id == id)
                .map_or_else(PersonaId::alfa, |u| u.persona.clone());
            self.emit(Command::ReleaseSpeaker {
                persona,
                utterance: id,
            });
        }
    }

    pub(crate) fn drop_proactive_pending(&mut self) {
        if self
            .s
            .pending
            .as_ref()
            .is_some_and(|p| p.proactive.is_some())
        {
            self.s.pending = None;
            self.notify(DialogNotice::ProactiveRejected {
                reason: ProactiveRejection::NotIdle,
            });
        }
    }

    pub(crate) fn on_response_ready(&mut self, persona: &PersonaId) {
        if self.s.phase != DialogPhase::Thinking || self.s.pending.is_some() {
            self.ignored("odpowiedź poza fazą myślenia");
            return;
        }
        let id = self.alloc();
        self.s.pending = Some(PendingSpeech {
            utterance: id,
            persona: persona.clone(),
            text: None,
            proactive: None,
        });
        self.emit(Command::AcquireSpeaker {
            persona: persona.clone(),
            utterance: id,
        });
    }

    pub(crate) fn on_granted(&mut self, persona: &PersonaId, id: UtteranceId) {
        let Some(p) = self.s.pending.clone().filter(|p| p.utterance == id) else {
            // Przydział nieaktualny (np. po stopie) — oddaj głośnik od razu.
            self.emit(Command::ReleaseSpeaker {
                persona: persona.clone(),
                utterance: id,
            });
            return;
        };
        self.s.pending = None;
        self.s.speaker_held = Some(id);
        if self.s.filler_played && self.s.phase == DialogPhase::Thinking {
            self.emit(Command::StopFiller);
        }
        self.s.utterance = Some(Utterance {
            id,
            persona: p.persona.clone(),
            chunks: Vec::new(),
            played_ms: 0,
            proactive: p.proactive.clone(),
        });
        self.s.phase = DialogPhase::Speaking;
        self.s.thinking_since_ms = None;
        match p.proactive {
            Some(label) => self.emit(Command::SpeakProactive {
                utterance: id,
                persona: p.persona,
                text: p.text.unwrap_or_default(),
                label,
            }),
            None => self.emit(Command::StartTts {
                utterance: id,
                persona: p.persona,
            }),
        }
        if self.s.vad_active {
            self.begin_barge_in(true);
        }
    }

    pub(crate) fn on_denied(&mut self, id: UtteranceId) {
        let Some(p) = self.s.pending.clone().filter(|p| p.utterance == id) else {
            return;
        };
        if p.proactive.is_some() {
            self.s.pending = None;
            self.notify(DialogNotice::ProactiveRejected {
                reason: ProactiveRejection::SpeakerBusy,
            });
        } else {
            self.notify(DialogNotice::SpeakerBusy { utterance: id });
        }
    }

    pub(crate) fn on_speaker_released(&mut self) {
        if let Some(p) = self.s.pending.clone() {
            self.emit(Command::AcquireSpeaker {
                persona: p.persona,
                utterance: p.utterance,
            });
        }
    }

    pub(crate) fn on_chunk(&mut self, id: UtteranceId, text: &str, audio_ms: u64) {
        if !self.s.is_speaking(id) {
            return;
        }
        if let Some(u) = self.s.utterance.as_mut() {
            let (char_start, ms_start) = u.chunks.last().map_or((0, 0), |c| {
                (
                    c.char_start + c.text.chars().count() + 1,
                    c.ms_start + c.audio_ms,
                )
            });
            u.chunks.push(SpokenChunk {
                text: text.to_owned(),
                audio_ms,
                char_start,
                ms_start,
                marks: None,
                mark_source: None,
            });
        }
    }

    pub(crate) fn on_marks(
        &mut self,
        id: UtteranceId,
        chunk: usize,
        marks: &[WordMark],
        source: MarkSource,
    ) {
        if !self.s.is_speaking(id) {
            return;
        }
        if let Some(c) = self
            .s
            .utterance
            .as_mut()
            .and_then(|u| u.chunks.get_mut(chunk))
        {
            // Znaczniki z TTS mają pierwszeństwo przed alignmentem.
            if c.mark_source == Some(MarkSource::Tts) && source == MarkSource::Alignment {
                return;
            }
            c.marks = Some(marks.to_vec());
            c.mark_source = Some(source);
        }
    }

    pub(crate) fn on_progress(
        &mut self,
        id: UtteranceId,
        samples: u64,
        rate: u32,
        latency_ms: u64,
    ) {
        if !self.s.is_speaking(id) || rate == 0 {
            return;
        }
        if let Some(u) = self.s.utterance.as_mut() {
            let played =
                (samples.saturating_mul(1000) / u64::from(rate)).saturating_sub(latency_ms);
            u.played_ms = u.played_ms.max(played);
        }
    }

    pub(crate) fn on_finished(&mut self, id: UtteranceId) {
        if !self.s.is_speaking(id) {
            return;
        }
        let proactive = self
            .s
            .utterance
            .as_ref()
            .is_some_and(|u| u.proactive.is_some());
        if let Some(u) = self.s.utterance.as_mut() {
            u.played_ms = u
                .chunks
                .last()
                .map_or(u.played_ms, |c| c.ms_start + c.audio_ms);
        }
        self.release_speaker();
        self.restore_output();
        self.s.interruption = None;
        self.s.phase = match self.s.barge_in.take() {
            Some(b) => {
                self.s.user = UserTurn {
                    text: b.partial,
                    started_at_ms: Some(b.started_at_ms),
                };
                DialogPhase::UserSpeaking
            }
            None if proactive => DialogPhase::Idle,
            None => DialogPhase::Listening,
        };
    }

    pub(crate) fn on_proactive(&mut self, persona: &PersonaId, text: &str, label: &ProactiveLabel) {
        let reject = if self.m.cfg.proactive == ProactiveMode::Off {
            Some(ProactiveRejection::Disabled)
        } else if self.s.do_not_disturb {
            Some(ProactiveRejection::DoNotDisturb)
        } else if self.s.phase != DialogPhase::Idle || self.s.pending.is_some() || self.s.vad_active
        {
            Some(ProactiveRejection::NotIdle)
        } else {
            None
        };
        if let Some(reason) = reject {
            self.notify(DialogNotice::ProactiveRejected { reason });
            return;
        }
        let id = self.alloc();
        self.s.pending = Some(PendingSpeech {
            utterance: id,
            persona: persona.clone(),
            text: Some(text.to_owned()),
            proactive: Some(label.clone()),
        });
        self.emit(Command::AcquireSpeaker {
            persona: persona.clone(),
            utterance: id,
        });
    }

    /// Wznawia (od punktu cięcia albo od początku) jako nową wypowiedź.
    pub(crate) fn resume(
        &mut self,
        from: UtteranceId,
        persona: PersonaId,
        offset: usize,
        text: String,
    ) {
        if text.trim().is_empty() {
            self.ignored("nic do wznowienia");
            self.s.phase = DialogPhase::Listening;
            return;
        }
        let id = self.alloc();
        self.emit(Command::ResumeFrom {
            from,
            offset,
            text: text.clone(),
            utterance: id,
        });
        self.s.pending = Some(PendingSpeech {
            utterance: id,
            persona: persona.clone(),
            text: Some(text),
            proactive: None,
        });
        self.emit(Command::AcquireSpeaker {
            persona,
            utterance: id,
        });
        self.s.phase = DialogPhase::Thinking;
        self.s.thinking_since_ms = Some(self.now);
        self.s.filler_played = true;
    }
}
