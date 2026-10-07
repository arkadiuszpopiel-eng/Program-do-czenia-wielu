//! Komendy szybkie (`voice-cmd`) na transkrypcie: partial w trakcie mowy agentki działa jak
//! keyword-spotter („stop/czekaj” bez czekania na pauzę), partial w ciszy — z pauzą `settle_ms`,
//! final — z adresowaniem; trafienie trafia do automatu i na magistralę z czasem reakcji.

use voice_cmd_contract::{AgentActivity, CmdDecision, CmdHit, CmdInput, CmdSource, Token};
use voice_dialog_contract::DialogEvent;
use voice_pipeline_contract::PipelineEvent;
use voice_stt_contract::Transcript;
use voice_turn_contract::TurnEvent;

use crate::pipeline::Pipeline;

/// Tokeny transkryptu w czasie potoku (słowa STT względem początku wypowiedzi).
pub(crate) fn tokens(start_ms: u64, t: &Transcript) -> Vec<Token> {
    t.words
        .iter()
        .map(|w| Token {
            text: w.text.clone(),
            start_ms: start_ms + u64::from(w.start_ms),
            end_ms: start_ms + u64::from(w.end_ms),
            confidence: w.confidence,
        })
        .collect()
}

/// Tekst tury: przeniesienie z poprzedniej wypowiedzi + bieżący transkrypt.
pub(crate) fn join(carry: &str, text: &str) -> String {
    format!("{carry} {text}").trim().to_owned()
}

impl Pipeline {
    pub(crate) fn activity(&self) -> AgentActivity {
        self.dialog.state().phase.agent_activity()
    }

    pub(crate) fn on_partial(&mut self, t: &Transcript) {
        let activity = self.activity();
        let (input, fired) = {
            let Some(u) = self.st.user.as_ref() else {
                return;
            };
            let input = CmdInput {
                tokens: tokens(u.start_ms, t),
                source: if activity == AgentActivity::Silent {
                    CmdSource::Partial
                } else {
                    CmdSource::Kws
                },
                activity,
                now_ms: self.st.now_ms,
                prev_speech_end_ms: u.prev_speech_end,
                addressed: self.st.addressed,
            };
            (input, u.cmd_fired.is_some())
        };
        if !fired {
            let decision = self.commands.recognize(&input);
            self.on_cmd_decision(decision, input);
        }
        if self.st.user.as_ref().is_some_and(|u| u.cmd_fired.is_some()) {
            return;
        }
        let text = join(&self.st.carry, &t.text);
        self.st.partial.clone_from(&text);
        self.turn.observe(&TurnEvent::Partial {
            at_ms: self.st.now_ms,
            text: text.clone(),
        });
        self.publish(&PipelineEvent::Transcript {
            text: text.clone(),
            is_final: false,
        });
        self.dialog_event(DialogEvent::UserPartial { text });
    }

    pub(crate) fn on_cmd_decision(&mut self, decision: CmdDecision, input: CmdInput) {
        match decision {
            CmdDecision::Hit(hit) => {
                let kws = input.source == CmdSource::Kws;
                if kws && !hit.command.is_barge_in() {
                    return;
                }
                if let Some(u) = self.st.user.as_mut() {
                    u.cmd_fired = Some(hit.command.clone());
                }
                self.fire_command(&hit);
            }
            CmdDecision::Pending { recheck_at_ms } => {
                if let Some(u) = self.st.user.as_mut() {
                    u.cmd_recheck = Some((recheck_at_ms, input));
                }
            }
            CmdDecision::Ignored { command, reason } => {
                self.outbox.push(core_bus_contract::Event::new(
                    voice_cmd_contract::event_kind(voice_cmd_contract::EVENT_IGNORED),
                    core_bus_contract::Level::Debug,
                    serde_json::json!({ "command": command, "reason": reason }),
                ));
            }
            CmdDecision::NoMatch => {}
        }
    }

    pub(crate) fn fire_command(&mut self, hit: &CmdHit) {
        let reaction_ms = self.st.now_ms.saturating_sub(hit.at_ms);
        self.outbox.push(core_bus_contract::Event::new(
            voice_cmd_contract::event_kind(voice_cmd_contract::EVENT_DETECTED),
            core_bus_contract::Level::Info,
            serde_json::json!({
                "command": hit.command,
                "source": hit.source,
                "confidence": hit.confidence,
                "reaction_ms": reaction_ms,
            }),
        ));
        self.dialog_event(DialogEvent::Command {
            command: hit.command.clone(),
        });
    }
}
