//! Runner offline zestawu F2: nagranie pozycji przez te same kontrakty co potok — VAD (energia,
//! automat z kontraktu), STT (partiale co 100 ms jak keyword-spotter + final), komendy szybkie
//! i automat dialogu w stanie „agentka mówi” (backchannel vs przerwanie, intencja). Silniki podaje
//! wywołujący: na CI atrapy, na maszynie użytkownika prawdziwe modele (whisper.cpp, Grammar…).

use std::time::Duration;

use personas_contract::PersonaId;
use voice_audio_contract::{Frame, MediaTime, PIPELINE_RATE};
use voice_cmd_contract::{AgentActivity, CmdDecision, CmdInput, CmdSource, CommandRecognizer};
use voice_dialog_contract::{
    ActivationSource, Command, DialogAutomaton, DialogEvent, DialogNotice, DialogState, UtteranceId,
};
use voice_stt_contract::{Stt, Transcript, UtteranceId as SttUtterance};
use voice_vad_contract::{EnergyDetector, VadCfg, VadEngine, VadEvent, VadMachine};

use crate::eval::manifest::{ItemKind, ManifestEntry, TtsContext};
use crate::eval::results::ItemResult;

/// Silniki oceny (kontrakty).
pub struct Engines<'a> {
    /// Rozpoznawanie mowy.
    pub stt: &'a dyn Stt,
    /// Komendy szybkie.
    pub commands: &'a dyn CommandRecognizer,
    /// Automat dialogu (produkcyjnie `voice_dialog_contract::default_machine()`).
    pub dialog: &'a dyn DialogAutomaton,
}

/// Co ile ms partial na żądanie (jak barge-in w potoku).
const PARTIAL_EVERY_MS: u64 = 100;
const FRAME_MS: u64 = 10;

/// Automat dialogu doprowadzony do mowy agentki (długa wypowiedź w toku).
struct Dialog<'a> {
    automaton: &'a dyn DialogAutomaton,
    state: DialogState,
    log: Vec<Command>,
    /// Odtworzone ms wypowiedzi agentki na początku pozycji.
    base_ms: u64,
}

/// Domyślna wypowiedź agentki w tle (pozycje bez `tts`).
const DEFAULT_TTS: &str = "Jutro rano masz spotkanie zespołu, potem przegląd budżetu, po południu \
    rozmowę z klientem, a wieczorem urodziny siostry. ";

impl Dialog<'_> {
    fn feed(&mut self, event: DialogEvent, now: u64) {
        let mut queue = vec![event];
        while let Some(ev) = queue.pop() {
            let t = self.automaton.step(&self.state, &ev, now);
            self.state = t.state;
            for c in t.commands {
                if let Command::AcquireSpeaker { persona, utterance } = &c {
                    queue.push(DialogEvent::SpeakerGranted {
                        persona: persona.clone(),
                        utterance: *utterance,
                    });
                }
                self.log.push(c);
            }
        }
    }

    fn speaking<'a>(automaton: &'a dyn DialogAutomaton, tts: Option<&TtsContext>) -> Dialog<'a> {
        let mut d = Dialog {
            automaton,
            state: DialogState::default(),
            log: Vec::new(),
            base_ms: 0,
        };
        d.feed(
            DialogEvent::Activate {
                source: ActivationSource::PushToTalk,
            },
            0,
        );
        d.feed(
            DialogEvent::UserTyped {
                text: "Opowiedz mi o planie dnia.".into(),
            },
            0,
        );
        d.feed(
            DialogEvent::ResponseReady {
                persona: PersonaId::alfa(),
            },
            0,
        );
        let utterance = d.state.utterance.as_ref().map_or(UtteranceId(1), |u| u.id);
        let (text, audio_ms) = match tts {
            Some(t) => {
                d.base_ms = t.played_at_start_ms;
                (t.text.clone(), t.audio_ms)
            }
            None => (DEFAULT_TTS.repeat(4), 60_000),
        };
        d.feed(
            DialogEvent::TtsChunkQueued {
                utterance,
                text,
                audio_ms,
            },
            0,
        );
        d.log.clear();
        d
    }

    fn stopped(&self) -> bool {
        self.log
            .iter()
            .any(|c| matches!(c, Command::StopTts { .. }))
    }

    fn progress(&mut self, now: u64) {
        if let Some(u) = self.state.utterance.as_ref().map(|u| u.id) {
            self.feed(
                DialogEvent::PlaybackProgress {
                    utterance: u,
                    played_samples: (self.base_ms + now) * 48,
                    sample_rate: 48_000,
                    device_latency_ms: 0,
                },
                now,
            );
        }
        self.feed(DialogEvent::Tick, now);
    }
}

fn kws(
    commands: &dyn CommandRecognizer,
    t: &Transcript,
    now: u64,
    activity: AgentActivity,
) -> Option<voice_cmd_contract::CmdHit> {
    let tokens = t
        .words
        .iter()
        .map(|w| voice_cmd_contract::Token {
            text: w.text.clone(),
            start_ms: u64::from(w.start_ms),
            end_ms: u64::from(w.end_ms),
            confidence: w.confidence,
        })
        .collect();
    let input = CmdInput {
        tokens,
        source: if t.is_final {
            CmdSource::Final
        } else {
            CmdSource::Kws
        },
        activity,
        now_ms: now,
        prev_speech_end_ms: None,
        addressed: true,
    };
    match commands.recognize(&input) {
        CmdDecision::Hit(h) => Some(h),
        _ => None,
    }
}

/// Ocena jednej pozycji (audio mono 16 kHz). `utterance` — identyfikator wypowiedzi STT.
pub async fn run_item(
    e: &Engines<'_>,
    entry: &ManifestEntry,
    pcm: &[f32],
    utterance: u64,
) -> Result<ItemResult, String> {
    let id = SttUtterance(utterance);
    let barge = matches!(entry.kind, ItemKind::Backchannel | ItemKind::Interruption);
    let command = entry.kind == ItemKind::Command;
    let activity = if command || barge {
        AgentActivity::Speaking
    } else {
        AgentActivity::Silent
    };
    let mut dialog = barge.then(|| Dialog::speaking(e.dialog, entry.tts.as_ref()));
    let mut vad = VadMachine::new(VadCfg {
        engine: VadEngine::Energy,
        ..VadCfg::default()
    });
    let mut energy = EnergyDetector::new();
    let mut result = ItemResult {
        id: entry.id.clone(),
        ..ItemResult::default()
    };
    let mut speech_from: Option<u64> = None;
    e.stt
        .start_utterance(id)
        .await
        .map_err(|err| err.to_string())?;
    let per = (PIPELINE_RATE as usize * FRAME_MS as usize) / 1_000;
    for (i, chunk) in pcm.chunks(per).enumerate() {
        let now = (i as u64 + 1) * FRAME_MS;
        let ts = MediaTime::from_ms(i as u64 * FRAME_MS);
        let frame = Frame::mono(chunk.to_vec(), PIPELINE_RATE, ts);
        e.stt
            .push(id, &frame)
            .await
            .map_err(|err| err.to_string())?;
        if let Some(ev) = vad.step(ts, Duration::from_millis(FRAME_MS), energy.prob(chunk)) {
            match ev {
                VadEvent::SpeechStart { ts, .. } => {
                    speech_from.get_or_insert(ts.as_ms());
                    if let Some(d) = dialog.as_mut() {
                        d.feed(DialogEvent::VadSpeechStart, now);
                    }
                }
                VadEvent::SpeechEnd { .. } => {
                    if let Some(d) = dialog.as_mut() {
                        d.feed(DialogEvent::VadSpeechEnd, now);
                    }
                }
            }
        }
        let due =
            speech_from.is_some_and(|s| now > s && (now - s).is_multiple_of(PARTIAL_EVERY_MS));
        // Partiale tylko do decyzji (komenda / twardy stop) — przy prawdziwym STT każdy kosztuje.
        let undecided =
            (command && result.command.is_none()) || dialog.as_ref().is_some_and(|d| !d.stopped());
        if due
            && undecided
            && let Some(t) = e.stt.partial_now(id).await.map_err(|err| err.to_string())?
        {
            if result.command.is_none()
                && let Some(h) = kws(e.commands, &t, now, activity)
            {
                result.command = Some(h.command.kind());
                result.reaction_ms =
                    Some(now.saturating_sub(entry.onset_ms.or(speech_from).unwrap_or(0)));
            }
            if let Some(d) = dialog.as_mut() {
                d.feed(
                    DialogEvent::UserPartial {
                        text: t.text.clone(),
                    },
                    now,
                );
            }
        }
        if let Some(d) = dialog.as_mut() {
            d.progress(now);
        }
    }
    let end = pcm.len() as u64 * 1_000 / u64::from(PIPELINE_RATE);
    let final_t = e
        .stt
        .end_utterance(id)
        .await
        .map_err(|err| err.to_string())?;
    if command
        && result.command.is_none()
        && let Some(h) = kws(e.commands, &final_t, end, activity)
    {
        result.command = Some(h.command.kind());
        result.reaction_ms = Some(end.saturating_sub(entry.onset_ms.or(speech_from).unwrap_or(0)));
    }
    if let Some(d) = dialog.as_mut() {
        d.feed(
            DialogEvent::UserPartial {
                text: final_t.text.clone(),
            },
            end,
        );
        d.feed(DialogEvent::TurnEnded, end);
        result.interrupted = Some(d.stopped());
        result.intent = d.log.iter().find_map(|c| match c {
            Command::Notify {
                notice: DialogNotice::IntentClassified { intent, .. },
            } => Some(*intent),
            _ => None,
        });
        if entry.tts.is_some() {
            result.heard_words = d.log.iter().find_map(|c| match c {
                Command::Notify {
                    notice: DialogNotice::Interrupted { heard },
                } => Some(heard.words),
                _ => None,
            });
        }
    }
    result.hypothesis = Some(final_t.text);
    Ok(result)
}
