//! Kontrakt współdzielony, manifest i przypadki brzegowe implementacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_registry_contract::ModuleManifest;
use voice_cmd_contract::contract_tests::{self, utterance};
use voice_cmd_contract::{
    AgentActivity, CmdDecision, CommandKind, CommandRecognizer, Grammar, GrammarRule, IgnoreReason,
    Token, VoiceCommand,
};
use voice_cmd_impl::{GrammarRecognizer, MODULE_TOML};
use voice_persona_contract::PersonaId;

#[test]
fn contract_suite() {
    contract_tests::run_all(GrammarRecognizer::default);
}

#[test]
fn manifest_is_valid() {
    let m = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "voice-cmd");
    assert_eq!(m.provides[0].to_string(), "voice-cmd-contract@1");
}

#[test]
fn addressing_by_name_and_low_confidence() {
    let r = GrammarRecognizer::default();
    let mut named = utterance("Delto, głośniej", AgentActivity::Silent);
    named.addressed = false;
    let hit = r.recognize(&named).hit().cloned().unwrap();
    assert!(hit.addressed && hit.command == VoiceCommand::VolumeUp);
    let mut noisy = utterance("stop", AgentActivity::Speaking);
    noisy.tokens[0].confidence = 0.3;
    assert!(matches!(
        r.recognize(&noisy),
        CmdDecision::Ignored {
            reason: IgnoreReason::LowConfidence { .. },
            ..
        }
    ));
}

#[test]
fn asr_token_with_many_words_and_standalone_flag() {
    let r = GrammarRecognizer::default();
    let mut input = utterance("x", AgentActivity::Silent);
    input.tokens = vec![Token::new("przełącz na Gamę.", 1000, 1900)];
    input.now_ms = 2700;
    let hit = r.recognize(&input).hit().cloned().unwrap();
    assert_eq!(
        hit.command,
        VoiceCommand::SwitchPersona {
            persona: PersonaId::gama()
        }
    );
    assert!(hit.standalone);
    let mut glued = utterance("stop", AgentActivity::Speaking);
    glued.prev_speech_end_ms = Some(990);
    assert!(!r.recognize(&glued).hit().unwrap().standalone);
}

#[test]
fn custom_grammar_is_used() {
    let mut g = Grammar::default_pl_en();
    g.rules.push(GrammarRule {
        command: CommandKind::Stop,
        phrases: vec!["basta".into()],
    });
    let r = GrammarRecognizer::new(g.clone());
    assert_eq!(
        r.recognize(&utterance("basta", AgentActivity::Speaking))
            .hit()
            .map(|h| h.command.clone()),
        Some(VoiceCommand::Stop)
    );
    assert_eq!(r.grammar(), g);
    assert_eq!(
        GrammarRecognizer::default().recognize(&utterance("basta", AgentActivity::Speaking)),
        CmdDecision::NoMatch
    );
}

#[test]
fn switch_needs_known_persona() {
    let r = GrammarRecognizer::default();
    assert_eq!(
        r.recognize(&utterance("przełącz na Omegę", AgentActivity::Silent)),
        CmdDecision::NoMatch
    );
    assert_eq!(
        r.recognize(&utterance("przełącz na", AgentActivity::Silent)),
        CmdDecision::NoMatch
    );
}
