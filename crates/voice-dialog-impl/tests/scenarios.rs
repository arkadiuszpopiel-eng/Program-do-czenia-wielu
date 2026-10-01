//! Scenariusze automatu (wirtualny zegar): przerwanie i intencje, wznawianie, pauza, powtórz,
//! „nie” z `voice-cmd`, szum, fillery, mowa proaktywna, przerwanie tekstem w myśleniu, mikrofon.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::Sim;
use personas_contract::PersonaId;
use voice_cmd_contract::VoiceCommand;
use voice_dialog_contract::{
    ActivationSource, Command, DialogEvent, DialogNotice, DialogPhase, InterruptIntent,
    ProactiveLabel, ProactiveRejection, TurnSource,
};
use voice_dialog_impl::default_machine;

fn speaking_sim() -> (
    Sim<voice_dialog_impl::DialogMachine<voice_dialog_impl::HeuristicClassifier>>,
    voice_dialog_contract::UtteranceId,
) {
    let mut sim = Sim::new(default_machine());
    let id = sim.speak(1000, "Jaka będzie pogoda?");
    sim.queue_words(
        id,
        &[
            "Jutro",
            "będzie",
            "słonecznie",
            "i",
            "ciepło",
            "na",
            "całym",
            "południu.",
        ],
        300,
        50,
        true,
    );
    sim.play(id, 1900, 2600, 20);
    (sim, id)
}

fn barge(sim: &mut Sim<impl voice_dialog_contract::DialogAutomaton>, t: u64, text: &str) {
    sim.at(t, DialogEvent::VadSpeechStart);
    sim.at(t + 150, DialogEvent::UserPartial { text: text.into() });
    sim.tick_until(t + 260, 10);
    sim.at(t + 900, DialogEvent::VadSpeechEnd);
    sim.at(t + 1200, DialogEvent::TurnEnded);
}

#[test]
fn correction_submits_turn_with_heard_prefix_and_intent() {
    let (mut sim, _) = speaking_sim();
    barge(&mut sim, 2600, "nie, chodziło mi o jutro rano");
    let submit = sim.since(2600).into_iter().find_map(|c| match c {
        Command::SubmitTurn {
            heard_prefix,
            interrupted_intent,
            text,
            ..
        } => Some((heard_prefix.clone(), *interrupted_intent, text.clone())),
        _ => None,
    });
    let (heard, intent, text) = submit.unwrap();
    assert_eq!(intent, Some(InterruptIntent::Correction));
    assert_eq!(text, "nie, chodziło mi o jutro rano");
    let heard = heard.unwrap();
    assert_eq!(heard.text, "Jutro będzie");
    assert!(!heard.approximate);
    assert_eq!(sim.s.phase, DialogPhase::Thinking);
}

#[test]
fn continue_resumes_from_cut_point() {
    let (mut sim, id) = speaking_sim();
    barge(&mut sim, 2600, "mhm, mów dalej");
    let resume = sim.since(2600).into_iter().find_map(|c| match c {
        Command::ResumeFrom {
            from,
            offset,
            text,
            utterance,
        } => Some((*from, *offset, text.clone(), *utterance)),
        _ => None,
    });
    let (from, offset, text, new_id) = resume.unwrap();
    assert_eq!((from, offset), (id, "Jutro będzie".chars().count()));
    assert_eq!(text, "słonecznie i ciepło na całym południu.");
    assert_ne!(new_id, id);
    let cmds = sim.at(
        4000,
        DialogEvent::SpeakerGranted {
            persona: PersonaId::alfa(),
            utterance: new_id,
        },
    );
    assert!(cmds.contains(&Command::StartTts {
        utterance: new_id,
        persona: PersonaId::alfa()
    }));
}

#[test]
fn wait_keeps_generation_and_resume_command_continues() {
    let (mut sim, id) = speaking_sim();
    let cmds = sim.at(
        2600,
        DialogEvent::Command {
            command: VoiceCommand::Wait,
        },
    );
    assert!(cmds.contains(&Command::StopTts { utterance: id }));
    assert!(!cmds.contains(&Command::CancelGeneration));
    assert_eq!(sim.s.phase, DialogPhase::Listening);
    let cmds = sim.at(
        5000,
        DialogEvent::Command {
            command: VoiceCommand::Resume,
        },
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, Command::ResumeFrom { from, .. } if *from == id))
    );
    assert_eq!(sim.s.phase, DialogPhase::Thinking);
}

#[test]
fn repeat_restarts_last_utterance_from_beginning() {
    let (mut sim, id) = speaking_sim();
    sim.at(5000, DialogEvent::ResponseFinished { utterance: id });
    let cmds = sim.at(
        6000,
        DialogEvent::Command {
            command: VoiceCommand::Repeat,
        },
    );
    assert!(cmds.iter().any(|c| matches!(c, Command::ResumeFrom { from, offset: 0, text, .. } if *from == id && text.starts_with("Jutro będzie"))));
}

#[test]
fn standalone_nie_from_voice_cmd_interrupts_only_while_speaking() {
    let (mut sim, id) = speaking_sim();
    let cmds = sim.at(
        2600,
        DialogEvent::Command {
            command: VoiceCommand::No,
        },
    );
    assert!(cmds.contains(&Command::StopTts { utterance: id }));
    assert_eq!(sim.s.phase, DialogPhase::Interrupted);
    let mut idle = Sim::new(default_machine());
    idle.at(
        0,
        DialogEvent::Activate {
            source: ActivationSource::PushToTalk,
        },
    );
    let cmds = idle.at(
        10,
        DialogEvent::Command {
            command: VoiceCommand::No,
        },
    );
    assert!(cmds.iter().all(|c| matches!(
        c,
        Command::Notify {
            notice: DialogNotice::Ignored { .. }
        }
    )));
}

#[test]
fn short_noise_only_ducks_and_restores() {
    let (mut sim, _) = speaking_sim();
    sim.at(2600, DialogEvent::VadSpeechStart);
    let cmds = sim.at(2650, DialogEvent::VadSpeechEnd);
    assert!(cmds.contains(&Command::RestoreOutput));
    sim.tick_until(3200, 10);
    assert_eq!(sim.count(|c| matches!(c, Command::StopTts { .. })), 0);
    assert_eq!(sim.s.phase, DialogPhase::Speaking);
}

#[test]
fn filler_after_thinking_delay_and_stopped_on_speech() {
    let mut sim = Sim::new(default_machine());
    sim.at(
        0,
        DialogEvent::Activate {
            source: ActivationSource::PushToTalk,
        },
    );
    sim.at(100, DialogEvent::VadSpeechStart);
    sim.at(
        400,
        DialogEvent::UserPartial {
            text: "Zrób raport.".into(),
        },
    );
    sim.at(600, DialogEvent::VadSpeechEnd);
    sim.at(800, DialogEvent::TurnEnded);
    sim.tick_until(2100, 100);
    assert_eq!(sim.first(|c| *c == Command::PlayFiller), Some(2000));
    let cmds = sim.at(
        2200,
        DialogEvent::ResponseReady {
            persona: PersonaId::gama(),
        },
    );
    let id = cmds
        .iter()
        .find_map(|c| match c {
            Command::AcquireSpeaker { utterance, .. } => Some(*utterance),
            _ => None,
        })
        .unwrap();
    let cmds = sim.at(
        2200,
        DialogEvent::SpeakerGranted {
            persona: PersonaId::gama(),
            utterance: id,
        },
    );
    assert!(cmds.contains(&Command::StopFiller));
    assert_eq!(sim.count(|c| *c == Command::PlayFiller), 1);
}

#[test]
fn proactive_respects_dnd_and_idle_and_returns_to_idle() {
    let label = ProactiveLabel {
        who: PersonaId::beta(),
        reason: "spotkanie za 5 minut".into(),
    };
    let req = || DialogEvent::ProactiveRequest {
        persona: PersonaId::beta(),
        text: "Za pięć minut spotkanie.".into(),
        label: label.clone(),
    };
    let mut sim = Sim::new(default_machine());
    sim.at(
        0,
        DialogEvent::Command {
            command: VoiceCommand::DoNotDisturb,
        },
    );
    let cmds = sim.at(10, req());
    assert!(cmds.contains(&Command::Notify {
        notice: DialogNotice::ProactiveRejected {
            reason: ProactiveRejection::DoNotDisturb
        }
    }));
    sim.at(20, DialogEvent::SetDoNotDisturb { enabled: false });
    let cmds = sim.at(30, req());
    let id = cmds
        .iter()
        .find_map(|c| match c {
            Command::AcquireSpeaker { utterance, .. } => Some(*utterance),
            _ => None,
        })
        .unwrap();
    sim.at(
        40,
        DialogEvent::SpeakerGranted {
            persona: PersonaId::beta(),
            utterance: id,
        },
    );
    sim.at(
        50,
        DialogEvent::TtsChunkQueued {
            utterance: id,
            text: "Za pięć minut spotkanie.".into(),
            audio_ms: 1500,
        },
    );
    sim.at(1600, DialogEvent::ResponseFinished { utterance: id });
    assert_eq!(sim.s.phase, DialogPhase::Idle);
    let mut busy = Sim::new(default_machine());
    let cmds = busy.at(0, req());
    let id = cmds
        .iter()
        .find_map(|c| match c {
            Command::AcquireSpeaker { utterance, .. } => Some(*utterance),
            _ => None,
        })
        .unwrap();
    let cmds = busy.at(
        5,
        DialogEvent::SpeakerDenied {
            persona: PersonaId::beta(),
            utterance: id,
        },
    );
    assert!(cmds.contains(&Command::Notify {
        notice: DialogNotice::ProactiveRejected {
            reason: ProactiveRejection::SpeakerBusy
        }
    }));
    assert!(busy.s.pending.is_none());
}

#[test]
fn typed_during_thinking_cancels_and_submits_text_turn() {
    let mut sim = Sim::new(default_machine());
    sim.at(
        0,
        DialogEvent::UserTyped {
            text: "Napisz maila do Ani.".into(),
        },
    );
    assert_eq!(sim.s.phase, DialogPhase::Thinking);
    let cmds = sim.at(
        500,
        DialogEvent::UserTyped {
            text: "i jeszcze dodaj załącznik".into(),
        },
    );
    assert!(cmds.contains(&Command::CancelGeneration));
    assert!(cmds.iter().any(|c| matches!(
        c,
        Command::SubmitTurn {
            source: TurnSource::Text,
            interrupted_intent: Some(InterruptIntent::Addition),
            ..
        }
    )));
}

#[test]
fn stop_cancel_mute_and_kill_switch() {
    let (mut sim, _) = speaking_sim();
    let cmds = sim.at(
        2600,
        DialogEvent::Command {
            command: VoiceCommand::Cancel,
        },
    );
    assert!(cmds.contains(&Command::CancelTask) && cmds.contains(&Command::CancelGeneration));
    assert_eq!(sim.s.phase, DialogPhase::Listening);
    let cmds = sim.at(
        2700,
        DialogEvent::Command {
            command: VoiceCommand::MuteMic,
        },
    );
    assert!(cmds.contains(&Command::StopListening));
    assert_eq!(sim.s.phase, DialogPhase::Idle);
    let cmds = sim.at(
        2800,
        DialogEvent::Command {
            command: VoiceCommand::VolumeUp,
        },
    );
    assert!(cmds.contains(&Command::ForwardCommand {
        command: VoiceCommand::VolumeUp
    }));
    let (mut sim, id) = speaking_sim();
    let cmds = sim.at(2600, DialogEvent::Deactivate);
    assert!(
        cmds.contains(&Command::StopTts { utterance: id })
            && cmds.contains(&Command::StopListening)
    );
    assert_eq!(sim.s.phase, DialogPhase::Idle);
}

#[test]
fn user_speaking_through_end_of_response_becomes_turn() {
    let (mut sim, id) = speaking_sim();
    sim.at(2600, DialogEvent::VadSpeechStart);
    sim.at(2650, DialogEvent::UserPartial { text: "mhm".into() });
    sim.at(2700, DialogEvent::ResponseFinished { utterance: id });
    assert_eq!(sim.s.phase, DialogPhase::UserSpeaking);
    assert_eq!(sim.s.user.text, "mhm");
    assert!(!sim.s.output_ducked);
}
