//! Wypowiedź użytkownika: STT (pre-roll, ramki, partial na żądanie przy barge-in, final z
//! ponowieniem), komendy szybkie (`voice-cmd`; w trakcie mowy agentki partiale działają jak
//! keyword-spotter), koniec tury (`voice-turn`), adresowanie po imieniu (`voice-wake`).

use voice_audio_contract::Frame;
use voice_cmd_contract::{CmdDecision, CmdInput, CmdSource};
use voice_dialog_contract::{DialogEvent, DialogPhase};
use voice_pipeline_contract::{PipelineEvent, SwitchSource};
use voice_stt_contract::{SttError, Transcript, UtteranceId};
use voice_turn_contract::{TurnDecision, TurnEvent};
use voice_wake_contract::{WakeEvent, WakeInput};

use crate::pipeline::Pipeline;
use crate::stt::{SttOp, SttOut};

/// Bieżąca wypowiedź użytkownika (jedna wypowiedź STT).
pub(crate) struct UserUtt {
    pub id: UtteranceId,
    pub start_ms: u64,
    pub frames: Vec<Frame>,
    pub pending: Vec<Frame>,
    pub stored_ms: u64,
    pub vad_active: bool,
    pub closing: bool,
    pub retried: bool,
    pub speech_end_ms: Option<u64>,
    pub eot_ms: Option<u64>,
    pub prev_speech_end: Option<u64>,
    pub cmd_fired: Option<voice_cmd_contract::VoiceCommand>,
    pub cmd_recheck: Option<(u64, CmdInput)>,
    pub next_barge_partial: u64,
}

impl Pipeline {
    /// Początek mowy: nowa wypowiedź STT z pre-rollem (albo wznowienie trwającej).
    pub(crate) fn open_user(&mut self, speech_ms: u64) {
        if let Some(u) = self.st.user.as_mut().filter(|u| !u.closing) {
            u.vad_active = true;
            return;
        }
        if let Some(u) = self.st.user.take() {
            // Poprzednia czeka na final — jej tekst trafi do „przeniesienia” bieżącej tury.
            self.st.closing = Some(u);
        }
        self.st.next_stt += 1;
        let id = UtteranceId(self.st.next_stt);
        self.stt.submit(SttOp::Start(id));
        let pre: Vec<Frame> = self.st.preroll.iter().cloned().collect();
        let start_ms = pre.first().map_or(speech_ms, |f| f.ts.as_ms());
        let mut u = UserUtt {
            id,
            start_ms,
            frames: Vec::new(),
            pending: Vec::new(),
            stored_ms: 0,
            vad_active: true,
            closing: false,
            retried: false,
            speech_end_ms: None,
            eot_ms: None,
            prev_speech_end: self.st.prev_speech_end,
            cmd_fired: None,
            cmd_recheck: None,
            next_barge_partial: self.st.now_ms + u64::from(self.cfg.barge_partial_ms),
        };
        for f in pre {
            Self::store(&mut u, f, self.cfg.max_utterance_ms);
        }
        self.st.user = Some(u);
    }

    fn store(u: &mut UserUtt, f: Frame, max_ms: u32) {
        let ms = u64::try_from(f.duration().as_millis()).unwrap_or(0);
        if u.stored_ms + ms <= u64::from(max_ms) {
            u.frames.push(f.clone());
            u.stored_ms += ms;
        }
        u.pending.push(f);
    }

    /// Ramka do trwającej wypowiedzi.
    pub(crate) fn user_frame(&mut self, f: &Frame) {
        let max = self.cfg.max_utterance_ms;
        if let Some(u) = self.st.user.as_mut().filter(|u| !u.closing) {
            Self::store(u, f.clone(), max);
        }
    }

    /// Wysyła zebrane ramki do STT (jedna operacja na krok).
    pub(crate) fn flush_user_push(&mut self) {
        if let Some(u) = self.st.user.as_mut().filter(|u| !u.pending.is_empty()) {
            let frames = std::mem::take(&mut u.pending);
            self.stt.submit(SttOp::Push(u.id, frames));
        }
    }

    /// Koniec tury: final STT.
    pub(crate) fn close_user(&mut self) {
        self.flush_user_push();
        let now = self.st.now_ms;
        if let Some(u) = self.st.user.as_mut().filter(|u| !u.closing) {
            u.closing = true;
            u.eot_ms = Some(now);
            u.cmd_recheck = None;
            self.stt.submit(SttOp::End(u.id));
        }
    }

    /// Timery wypowiedzi: koniec tury, partial barge-in, ponowna ocena komendy.
    pub(crate) fn user_timers(&mut self) {
        let now = self.st.now_ms;
        let state = self.dialog.state();
        let barge = matches!(state.phase, DialogPhase::Speaking | DialogPhase::Thinking)
            && state.barge_in.is_some();
        let Some(u) = self.st.user.as_mut().filter(|u| !u.closing) else {
            return;
        };
        if barge && u.vad_active && now >= u.next_barge_partial && !self.stt.partial_pending() {
            u.next_barge_partial = now + u64::from(self.cfg.barge_partial_ms);
            let (id, frames) = (u.id, std::mem::take(&mut u.pending));
            if !frames.is_empty() {
                self.stt.submit(SttOp::Push(id, frames));
            }
            self.stt.submit(SttOp::Partial(id));
        }
        if let Some((at, mut input)) = u.cmd_recheck.take() {
            if now >= at {
                input.now_ms = now;
                let decision = self.commands.recognize(&input);
                self.on_cmd_decision(decision, input);
            } else if let Some(u) = self.st.user.as_mut() {
                u.cmd_recheck = Some((at, input));
            }
        }
        let waiting = self
            .st
            .user
            .as_ref()
            .is_some_and(|u| !u.closing && !u.vad_active && u.speech_end_ms.is_some());
        if waiting {
            let decision = self.turn.decide(now, None);
            if let TurnDecision::EndOfTurn { .. } = decision {
                self.outbox.push(core_bus_contract::Event::new(
                    voice_turn_contract::event_kind(voice_turn_contract::EVENT_END),
                    core_bus_contract::Level::Debug,
                    serde_json::to_value(decision).unwrap_or_default(),
                ));
                self.close_user();
            }
        }
    }

    /// Wynik STT.
    pub(crate) fn on_stt_out(&mut self, out: SttOut) {
        match out {
            SttOut::Partial(id, t) => {
                if self
                    .st
                    .user
                    .as_ref()
                    .is_some_and(|u| u.id == id && !u.closing)
                {
                    self.on_partial(&t);
                }
            }
            SttOut::Final(id, result) => {
                let Some((u, was_closing_slot)) = self.take_closing(id) else {
                    return;
                };
                match result {
                    Ok(t) => self.on_final(&u, &t),
                    Err(e) => self.retry_or_drop(u, was_closing_slot, &e),
                }
            }
            SttOut::Failed(id, e) => self.restart_user(id, &e),
        }
    }

    fn take_closing(&mut self, id: UtteranceId) -> Option<(crate::user::UserUtt, bool)> {
        if self.st.closing.as_ref().is_some_and(|u| u.id == id) {
            return self.st.closing.take().map(|u| (u, true));
        }
        if self
            .st
            .user
            .as_ref()
            .is_some_and(|u| u.id == id && u.closing)
        {
            return self.st.user.take().map(|u| (u, false));
        }
        None
    }

    /// Błąd ramki/startu w trakcie wypowiedzi: nowa wypowiedź STT z całym buforem (bez utraty).
    fn restart_user(&mut self, id: UtteranceId, e: &SttError) {
        let next = self.st.next_stt + 1;
        let Some(u) = self
            .st
            .user
            .as_mut()
            .filter(|u| u.id == id && !u.closing && !u.retried)
        else {
            return;
        };
        u.retried = true;
        u.id = UtteranceId(next);
        u.pending.clone_from(&u.frames);
        self.st.next_stt = next;
        self.stt.submit(SttOp::Cancel(id));
        self.stt.submit(SttOp::Start(UtteranceId(next)));
        self.degraded("stt", format!("ponawiam wypowiedź po błędzie: {e}"));
    }

    /// Błąd finalu: jedno ponowienie z buforem wypowiedzi, potem rezygnacja (zgłoszona).
    fn retry_or_drop(&mut self, mut u: UserUtt, closing_slot: bool, e: &SttError) {
        if u.retried {
            self.degraded("stt", format!("wypowiedź utracona: {e}"));
            if self.st.user.is_none() {
                self.close_turn_silently();
            }
            return;
        }
        self.st.next_stt += 1;
        let id = UtteranceId(self.st.next_stt);
        u.retried = true;
        u.id = id;
        self.stt.submit(SttOp::Start(id));
        self.stt.submit(SttOp::Push(id, u.frames.clone()));
        self.stt.submit(SttOp::End(id));
        self.degraded(
            "stt",
            format!("ponawiam rozpoznanie wypowiedzi po błędzie: {e}"),
        );
        if closing_slot {
            self.st.closing = Some(u);
        } else {
            self.st.user = Some(u);
        }
    }

    /// Zamyka turę bez przekazania do modelu (komenda, szum).
    pub(crate) fn close_turn_silently(&mut self) {
        self.st.partial.clear();
        self.dialog_event(DialogEvent::UserPartial {
            text: String::new(),
        });
        self.dialog_event(DialogEvent::TurnEnded);
    }

    fn on_final(&mut self, u: &UserUtt, t: &Transcript) {
        let text = crate::cmd::join(&self.st.carry, &t.text);
        if self.st.user.is_some() {
            // Użytkownik mówi dalej — tekst łączy się z kolejną wypowiedzią tej samej tury.
            self.st.carry = text;
            return;
        }
        self.st.carry.clear();
        self.st.partial.clear();
        self.turn.observe(&TurnEvent::Reset);
        if text.is_empty() {
            self.close_turn_silently();
            return;
        }
        let input = CmdInput {
            tokens: crate::cmd::tokens(u.start_ms, t),
            source: CmdSource::Final,
            activity: self.activity(),
            now_ms: self.st.now_ms,
            prev_speech_end_ms: u.prev_speech_end,
            addressed: self.st.addressed,
        };
        if let CmdDecision::Hit(hit) = self.commands.recognize(&input) {
            // Najpierw zamknięcie tury (automat przyjmuje „wznów/powtórz” poza `UserSpeaking`),
            // potem komenda — o ile nie wykonał jej już keyword-spotter w trakcie mowy.
            self.close_turn_silently();
            if u.cmd_fired.as_ref() != Some(&hit.command) {
                self.fire_command(&hit);
            }
            return;
        }
        let events = self
            .wake
            .handle(WakeInput::Transcript { text: text.clone() });
        for ev in &events {
            if let WakeEvent::Addressed {
                persona,
                by_name: true,
            } = ev
            {
                self.switch_persona(persona.clone(), SwitchSource::Name);
            }
        }
        self.on_wake_events(events);
        self.st.pending_latency = Some(voice_pipeline_contract::TurnLatency {
            speech_end_ms: u.speech_end_ms,
            end_of_turn_ms: u.eot_ms,
            stt_final_ms: Some(self.st.now_ms),
            ..voice_pipeline_contract::TurnLatency::default()
        });
        self.publish(&PipelineEvent::Transcript {
            text: text.clone(),
            is_final: true,
        });
        self.dialog_event(DialogEvent::UserPartial { text });
        self.dialog_event(DialogEvent::TurnEnded);
    }
}
