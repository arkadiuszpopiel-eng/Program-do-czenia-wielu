//! Testy atrapy: kontrakt współdzielony + adnotacje.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_cmd_contract::contract_tests::{self, utterance};
use voice_cmd_contract::{AgentActivity, CmdDecision, CommandRecognizer, VoiceCommand};
use voice_cmd_fake::FakeRecognizer;

#[test]
fn contract_suite() {
    contract_tests::run_all(FakeRecognizer::default);
}

#[test]
fn scripted_annotations_win_and_calls_are_recorded() {
    let fake = FakeRecognizer::default();
    fake.script("Hej Alfa zrób kawę", CmdDecision::NoMatch);
    fake.script("abrakadabra", CmdDecision::Pending { recheck_at_ms: 42 });
    assert_eq!(
        fake.recognize(&utterance("abrakadabra", AgentActivity::Speaking)),
        CmdDecision::Pending { recheck_at_ms: 42 }
    );
    assert_eq!(
        fake.recognize(&utterance("hej alfa zrób kawę", AgentActivity::Silent)),
        CmdDecision::NoMatch
    );
    assert_eq!(fake.calls().len(), 2);
}

#[test]
fn exact_only_no_fuzzy() {
    let fake = FakeRecognizer::default();
    assert_eq!(
        fake.recognize(&utterance("pałza", AgentActivity::Speaking)),
        CmdDecision::NoMatch
    );
    assert_eq!(
        fake.recognize(&utterance(
            "Alfa, przełącz się na Omegę proszę",
            AgentActivity::Silent
        ))
        .hit()
        .map(|h| h.command.clone()),
        None
    );
    assert_eq!(
        fake.recognize(&utterance(
            "przełącz się na Gamę proszę",
            AgentActivity::Silent
        ))
        .hit()
        .map(|h| h.command.kind()),
        Some(voice_cmd_contract::CommandKind::SwitchPersona)
    );
    assert_eq!(
        fake.recognize(&utterance("dobra stop", AgentActivity::Speaking))
            .hit()
            .map(|h| h.command.clone()),
        Some(VoiceCommand::Stop)
    );
}
