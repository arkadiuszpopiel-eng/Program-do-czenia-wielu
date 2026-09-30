//! Właściwości automatu na losowych sekwencjach zdarzeń (proptest, stałe ziarno):
//! nigdy „mówi i słucha” bez duckingu, po `StopTts` brak dalszego audio tej wypowiedzi,
//! głośnik trzymany tylko w `Speaking`, ducking tylko w `Speaking`, mowa proaktywna tylko z `Idle` bez DND.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use proptest::prelude::*;
use voice_cmd_contract::VoiceCommand;
use voice_dialog_contract::{
    ActivationSource, Command, DialogAutomaton, DialogEvent, DialogPhase, DialogState,
    ProactiveLabel, UtteranceId,
};
use voice_dialog_impl::default_machine;
use voice_persona_contract::PersonaId;

#[derive(Debug, Clone)]
enum Op {
    Activate,
    Deactivate,
    VadStart,
    VadEnd,
    Partial(usize),
    TurnEnded,
    Cmd(usize),
    Typed(usize),
    Ready,
    Grant { stale: bool },
    Deny,
    Released,
    Chunk,
    Progress(u64),
    Finished,
    Proactive,
    Dnd(bool),
    Esc,
    Tick(u64),
}

const TEXTS: &[&str] = &[
    "mhm",
    "nie",
    "nie no, dobrze",
    "stop",
    "a co z pocztą?",
    "dalej",
    "i jeszcze to",
    "",
    "tak",
];
const CMDS: &[VoiceCommand] = &[
    VoiceCommand::Stop,
    VoiceCommand::Wait,
    VoiceCommand::Pause,
    VoiceCommand::Resume,
    VoiceCommand::Repeat,
    VoiceCommand::Cancel,
    VoiceCommand::No,
    VoiceCommand::StopAll,
    VoiceCommand::MuteMic,
    VoiceCommand::DoNotDisturb,
    VoiceCommand::VolumeUp,
];

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::Activate),
        Just(Op::Deactivate),
        Just(Op::VadStart),
        Just(Op::VadEnd),
        (0..TEXTS.len()).prop_map(Op::Partial),
        Just(Op::TurnEnded),
        (0..CMDS.len()).prop_map(Op::Cmd),
        (0..TEXTS.len()).prop_map(Op::Typed),
        Just(Op::Ready),
        any::<bool>().prop_map(|stale| Op::Grant { stale }),
        Just(Op::Deny),
        Just(Op::Released),
        Just(Op::Chunk),
        (0u64..5000).prop_map(Op::Progress),
        Just(Op::Finished),
        Just(Op::Proactive),
        any::<bool>().prop_map(Op::Dnd),
        Just(Op::Esc),
        (1u64..500).prop_map(Op::Tick),
    ]
}

fn current(s: &DialogState) -> UtteranceId {
    s.utterance.as_ref().map_or(UtteranceId(0), |u| u.id)
}

fn to_event(op: &Op, s: &DialogState, vad: bool) -> Option<DialogEvent> {
    let pending = s.pending.as_ref().map(|p| (p.persona.clone(), p.utterance));
    Some(match op {
        Op::Activate => DialogEvent::Activate {
            source: ActivationSource::PushToTalk,
        },
        Op::Deactivate => DialogEvent::Deactivate,
        Op::VadStart if !vad => DialogEvent::VadSpeechStart,
        Op::VadEnd if vad => DialogEvent::VadSpeechEnd,
        Op::VadStart | Op::VadEnd => return None,
        Op::Partial(i) => DialogEvent::UserPartial {
            text: TEXTS[*i].into(),
        },
        Op::TurnEnded => DialogEvent::TurnEnded,
        Op::Cmd(i) => DialogEvent::Command {
            command: CMDS[*i].clone(),
        },
        Op::Typed(i) => DialogEvent::UserTyped {
            text: TEXTS[*i].into(),
        },
        Op::Ready => DialogEvent::ResponseReady {
            persona: PersonaId::alfa(),
        },
        Op::Grant { stale } => match (pending, stale) {
            (Some((persona, utterance)), false) => {
                DialogEvent::SpeakerGranted { persona, utterance }
            }
            _ => DialogEvent::SpeakerGranted {
                persona: PersonaId::alfa(),
                utterance: UtteranceId(current(s).0.max(1)),
            },
        },
        Op::Deny => {
            let (persona, utterance) = pending?;
            DialogEvent::SpeakerDenied { persona, utterance }
        }
        Op::Released => DialogEvent::SpeakerReleased,
        Op::Chunk => DialogEvent::TtsChunkQueued {
            utterance: current(s),
            text: "Zdanie testowe.".into(),
            audio_ms: 900,
        },
        Op::Progress(ms) => DialogEvent::PlaybackProgress {
            utterance: current(s),
            played_samples: ms * 48,
            sample_rate: 48_000,
            device_latency_ms: 20,
        },
        Op::Finished => DialogEvent::ResponseFinished {
            utterance: current(s),
        },
        Op::Proactive => DialogEvent::ProactiveRequest {
            persona: PersonaId::beta(),
            text: "Przypomnienie.".into(),
            label: ProactiveLabel {
                who: PersonaId::beta(),
                reason: "test".into(),
            },
        },
        Op::Dnd(on) => DialogEvent::SetDoNotDisturb { enabled: *on },
        Op::Esc => DialogEvent::StopSpeech,
        Op::Tick(_) => DialogEvent::Tick,
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 768, failure_persistence: None, rng_seed: proptest::test_runner::RngSeed::Fixed(0xD1A1), ..ProptestConfig::default() })]

    #[test]
    fn invariants(ops in prop::collection::vec(op(), 1..120)) {
        let m = default_machine();
        let mut s = DialogState::default();
        let mut now = 0u64;
        let mut vad = false;
        let mut stopped: BTreeSet<UtteranceId> = BTreeSet::new();
        for op in &ops {
            if let Op::Tick(dt) = op { now += dt; } else { now += 10; }
            let Some(ev) = to_event(op, &s, vad) else { continue };
            let before = s.clone();
            let t = m.step(&s, &ev, now);
            match ev {
                DialogEvent::VadSpeechStart => vad = true,
                DialogEvent::VadSpeechEnd => vad = false,
                _ => {}
            }
            for c in &t.commands {
                match c {
                    Command::StopTts { utterance } => { stopped.insert(*utterance); }
                    Command::StartTts { utterance, .. } | Command::SpeakProactive { utterance, .. } => {
                        prop_assert!(!stopped.contains(utterance), "audio po StopTts dla {:?}", utterance);
                    }
                    Command::AcquireSpeaker { .. } if matches!(ev, DialogEvent::ProactiveRequest { .. }) => {
                        prop_assert!(before.phase == DialogPhase::Idle && !before.do_not_disturb && !vad, "mowa proaktywna poza Idle/DND");
                    }
                    _ => {}
                }
            }
            s = t.state;
            prop_assert_eq!(s.vad_active, vad);
            if s.phase == DialogPhase::Speaking && vad {
                prop_assert!(s.output_ducked, "UserSpeaking+Speaking bez duckingu");
            }
            if s.phase != DialogPhase::Speaking {
                prop_assert!(!s.output_ducked, "ducking poza Speaking");
                prop_assert!(s.speaker_held.is_none(), "głośnik trzymany poza Speaking");
            } else {
                let id = s.utterance.as_ref().unwrap().id;
                prop_assert_eq!(s.speaker_held, Some(id));
                prop_assert!(!stopped.contains(&id), "Speaking po StopTts tej wypowiedzi");
            }
        }
    }
}
