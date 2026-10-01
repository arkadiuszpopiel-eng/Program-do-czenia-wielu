//! Współdzielony test kontraktowy `DialogAutomaton` (feature `contract-tests`): niezmienniki,
//! które musi spełniać każdy automat (impl i fake), na wirtualnym zegarze.

use personas_contract::PersonaId;
use voice_cmd_contract::VoiceCommand;

use crate::{
    ActivationSource, Command, DialogAutomaton, DialogEvent, DialogPhase, DialogState,
    ProactiveLabel, TurnSource, UtteranceId, drive,
};

fn has(cmds: &[(u64, Command)], pred: impl Fn(&Command) -> bool) -> bool {
    cmds.iter().any(|(_, c)| pred(c))
}

/// Id wypowiedzi z pierwszego `AcquireSpeaker`.
pub fn acquired(cmds: &[(u64, Command)]) -> Option<UtteranceId> {
    cmds.iter().find_map(|(_, c)| match c {
        Command::AcquireSpeaker { utterance, .. } => Some(*utterance),
        _ => None,
    })
}

/// Doprowadza automat do `Speaking` (tura głosowa „Jaka jest pogoda?”, odpowiedź Alfy).
pub fn to_speaking<A: DialogAutomaton>(a: &A) -> (DialogState, UtteranceId, Vec<(u64, Command)>) {
    let (s, mut cmds) = drive(
        a,
        DialogState::default(),
        &[
            (
                0,
                DialogEvent::Activate {
                    source: ActivationSource::PushToTalk,
                },
            ),
            (100, DialogEvent::VadSpeechStart),
            (
                600,
                DialogEvent::UserPartial {
                    text: "Jaka jest pogoda?".into(),
                },
            ),
            (900, DialogEvent::VadSpeechEnd),
            (1200, DialogEvent::TurnEnded),
            (
                1500,
                DialogEvent::ResponseReady {
                    persona: PersonaId::alfa(),
                },
            ),
        ],
    );
    let id = acquired(&cmds).unwrap_or_else(|| panic!("brak AcquireSpeaker: {cmds:?}"));
    let (s, more) = drive(
        a,
        s,
        &[
            (
                1500,
                DialogEvent::SpeakerGranted {
                    persona: PersonaId::alfa(),
                    utterance: id,
                },
            ),
            (
                1510,
                DialogEvent::TtsChunkQueued {
                    utterance: id,
                    text: "Jutro będzie słonecznie i ciepło.".into(),
                    audio_ms: 2000,
                },
            ),
            (
                1800,
                DialogEvent::PlaybackProgress {
                    utterance: id,
                    played_samples: 4800,
                    sample_rate: 48_000,
                    device_latency_ms: 20,
                },
            ),
        ],
    );
    cmds.extend(more);
    (s, id, cmds)
}

/// Pełna ścieżka: słuchanie → tura → myślenie → mówienie → koniec.
pub fn happy_path<A: DialogAutomaton>(a: &A) {
    let (s, id, cmds) = to_speaking(a);
    assert!(has(&cmds, |c| *c == Command::StartListening));
    assert!(has(
        &cmds,
        |c| matches!(c, Command::SubmitTurn { text, source: TurnSource::Voice, heard_prefix: None, .. } if text == "Jaka jest pogoda?")
    ));
    assert!(has(
        &cmds,
        |c| matches!(c, Command::StartTts { utterance, .. } if *utterance == id)
    ));
    assert_eq!(s.phase, DialogPhase::Speaking);
    let (s, cmds) = drive(
        a,
        s,
        &[(3600, DialogEvent::ResponseFinished { utterance: id })],
    );
    assert!(has(
        &cmds,
        |c| matches!(c, Command::ReleaseSpeaker { utterance, .. } if *utterance == id)
    ));
    assert_eq!(s.phase, DialogPhase::Listening);
}

/// `Esc`/„stop mowy” zatrzymuje TTS i zwalnia głośnik; późniejsze zdarzenia tej wypowiedzi są ignorowane.
pub fn stop_speech_and_stale_events<A: DialogAutomaton>(a: &A) {
    let (s, id, _) = to_speaking(a);
    let (s, cmds) = drive(a, s, &[(2000, DialogEvent::StopSpeech)]);
    assert!(has(&cmds, |c| *c == Command::StopTts { utterance: id }));
    assert!(has(&cmds, |c| matches!(c, Command::ReleaseSpeaker { .. })));
    assert_ne!(s.phase, DialogPhase::Speaking);
    let (s, cmds) = drive(
        a,
        s,
        &[
            (
                2100,
                DialogEvent::TtsChunkQueued {
                    utterance: id,
                    text: "Reszta.".into(),
                    audio_ms: 500,
                },
            ),
            (
                2200,
                DialogEvent::PlaybackProgress {
                    utterance: id,
                    played_samples: 96_000,
                    sample_rate: 48_000,
                    device_latency_ms: 20,
                },
            ),
            (
                2300,
                DialogEvent::SpeakerGranted {
                    persona: PersonaId::alfa(),
                    utterance: id,
                },
            ),
        ],
    );
    assert_ne!(s.phase, DialogPhase::Speaking);
    assert!(!has(&cmds, |c| matches!(
        c,
        Command::StartTts { .. } | Command::RestoreOutput
    )));
}

/// Mowa użytkownika w `Speaking`: w tym samym kroku ducking albo stop; twardy stop ≤ 400 ms.
pub fn barge_in_ducks_then_stops<A: DialogAutomaton>(a: &A) {
    let (s, id, _) = to_speaking(a);
    let t = a.step(&s, &DialogEvent::VadSpeechStart, 2000);
    assert!(
        t.commands
            .iter()
            .any(|c| matches!(c, Command::DuckOutput { .. } | Command::StopTts { .. }))
    );
    let mut all: Vec<(u64, Command)> = t.commands.iter().map(|c| (2000, c.clone())).collect();
    let mut s = t.state;
    for now in (2010..=2400).step_by(10) {
        let ev = if now == 2150 {
            DialogEvent::UserPartial {
                text: "nie, chodziło mi o wtorek".into(),
            }
        } else {
            DialogEvent::Tick
        };
        let t = a.step(&s, &ev, now);
        s = t.state;
        all.extend(t.commands.into_iter().map(|c| (now, c)));
    }
    let stopped_at = all
        .iter()
        .find(|(_, c)| *c == Command::StopTts { utterance: id })
        .map(|(t, _)| *t);
    assert!(
        stopped_at.is_some_and(|t| t <= 2400),
        "brak twardego stopu ≤ 400 ms: {all:?}"
    );
    assert!(
        has(&all, |c| *c == Command::CancelGeneration),
        "twardy stop anuluje LLM"
    );
    assert_ne!(s.phase, DialogPhase::Speaking);
}

/// Mowa proaktywna tylko w `Idle`, nigdy w DND ani podczas mowy użytkownika; zawsze z etykietą.
pub fn proactive_rules<A: DialogAutomaton>(a: &A) {
    let label = ProactiveLabel {
        who: PersonaId::beta(),
        reason: "przypomnienie".into(),
    };
    let req = DialogEvent::ProactiveRequest {
        persona: PersonaId::beta(),
        text: "Spotkanie za 5 minut.".into(),
        label: label.clone(),
    };
    let (s, _) = drive(
        a,
        DialogState::default(),
        &[
            (
                0,
                DialogEvent::Activate {
                    source: ActivationSource::PushToTalk,
                },
            ),
            (10, DialogEvent::VadSpeechStart),
        ],
    );
    let (_, cmds) = drive(a, s, &[(20, req.clone())]);
    assert!(!has(&cmds, |c| matches!(
        c,
        Command::AcquireSpeaker { .. } | Command::SpeakProactive { .. }
    )));
    let (s, _) = drive(
        a,
        DialogState::default(),
        &[(0, DialogEvent::SetDoNotDisturb { enabled: true })],
    );
    let (_, cmds) = drive(a, s, &[(10, req.clone())]);
    assert!(!has(&cmds, |c| matches!(c, Command::AcquireSpeaker { .. })));
    let (s, cmds) = drive(a, DialogState::default(), &[(0, req)]);
    let id =
        acquired(&cmds).unwrap_or_else(|| panic!("proaktywna w Idle powinna prosić o głośnik"));
    let granted = DialogEvent::SpeakerGranted {
        persona: PersonaId::beta(),
        utterance: id,
    };
    let (s, cmds) = drive(a, s, &[(5, granted)]);
    assert!(has(
        &cmds,
        |c| matches!(c, Command::SpeakProactive { label: l, .. } if *l == label)
    ));
    assert_eq!(s.phase, DialogPhase::Speaking);
}

/// Przerwanie tekstem w `Speaking`: stop TTS + tura z composera.
pub fn typed_interrupt<A: DialogAutomaton>(a: &A) {
    let (s, id, _) = to_speaking(a);
    let (s, cmds) = drive(
        a,
        s,
        &[(
            2000,
            DialogEvent::UserTyped {
                text: "a co z pocztą?".into(),
            },
        )],
    );
    assert!(has(&cmds, |c| *c == Command::StopTts { utterance: id }));
    assert!(has(&cmds, |c| matches!(
        c,
        Command::SubmitTurn {
            source: TurnSource::Text,
            ..
        }
    )));
    assert_ne!(s.phase, DialogPhase::Speaking);
}

/// „Stop wszystko” → kill-switch i `Idle`.
pub fn kill_switch<A: DialogAutomaton>(a: &A) {
    let (s, _, _) = to_speaking(a);
    let (s, cmds) = drive(
        a,
        s,
        &[(
            2000,
            DialogEvent::Command {
                command: VoiceCommand::StopAll,
            },
        )],
    );
    assert!(has(&cmds, |c| *c == Command::KillSwitch));
    assert_eq!(s.phase, DialogPhase::Idle);
}

/// Uruchamia cały zestaw na automacie.
pub fn run_all<A: DialogAutomaton>(a: &A) {
    happy_path(a);
    stop_speech_and_stale_events(a);
    barge_in_ducks_then_stops(a);
    proactive_rules(a);
    typed_interrupt(a);
    kill_switch(a);
}
