//! Testy atrapy potoku: kontrakt + skrypt na osi czasu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use personas_contract::PersonaId;
use voice_dialog_contract::{DialogPhase, HeardPrefix, PrefixSource, UtteranceId};
use voice_pipeline_contract::{
    EVENT_HEARD_PREFIX, EVENT_PERSONA_SWITCHED, PipelineInput, Speaker, TurnLatency, VoicePipeline,
    contract_tests,
};
use voice_pipeline_fake::{FakePipeline, ScriptStep};
use voice_wake_contract::MicState;

#[tokio::test]
async fn contract_suite() {
    let mut p = FakePipeline::default();
    contract_tests::run_all(&mut p).await;
    assert!(
        p.events()
            .iter()
            .any(|e| e.kind.as_str() == EVENT_PERSONA_SWITCHED)
    );
}

#[tokio::test]
async fn script_drives_status_and_events() {
    let mut p = FakePipeline::default();
    p.script(30, ScriptStep::Phase(DialogPhase::Speaking));
    p.script(30, ScriptStep::Speaker(Speaker::Agent(PersonaId::beta())));
    p.script(20, ScriptStep::Partial("jaka pogoda".into()));
    p.script(
        50,
        ScriptStep::Heard(HeardPrefix {
            utterance: UtteranceId(1),
            chars: 5,
            words: 1,
            text: "Jutro".into(),
            approximate: false,
            source: PrefixSource::WordMarks,
        }),
    );
    p.script(
        50,
        ScriptStep::Latency(TurnLatency {
            turn: 1,
            speech_end_ms: Some(0),
            first_audio_ms: Some(900),
            ..TurnLatency::default()
        }),
    );
    p.script(60, ScriptStep::Level(-20.0));
    p.input(PipelineInput::Ptt { pressed: true });
    let r = p.step().await;
    assert_eq!(r.now_ms, 10);
    assert_eq!(p.status().mic, MicState::Listening);
    for _ in 0..5 {
        p.step().await;
    }
    let s = p.status();
    assert_eq!(s.phase, DialogPhase::Speaking);
    assert_eq!(s.speaker, Speaker::Agent(PersonaId::beta()));
    assert_eq!(s.partial, "jaka pogoda");
    assert_eq!(s.interruptions, 1);
    assert_eq!(s.latency.time_to_first_audio_ms(), Some(900));
    assert_eq!(s.level_db, -20.0);
    assert!(
        p.events()
            .iter()
            .any(|e| e.kind.as_str() == EVENT_HEARD_PREFIX)
    );
    p.input(PipelineInput::StopSpeech);
    p.input(PipelineInput::Typed { text: "hej".into() });
    p.input(PipelineInput::Deactivate);
    p.step().await;
    let s = p.status();
    assert_eq!(
        (s.phase, s.mic_open, s.turns),
        (DialogPhase::Idle, false, 1)
    );
    assert_eq!(p.inputs().len(), 4);
}
