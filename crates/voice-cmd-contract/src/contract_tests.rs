//! Współdzielony test kontraktowy `CommandRecognizer` (feature `contract-tests`), uruchamiany
//! na `voice-cmd-impl` i `voice-cmd-fake`.

use crate::{
    AgentActivity, CmdDecision, CmdInput, CmdSource, CommandRecognizer, IgnoreReason, Token,
    VoiceCommand,
};
use personas_contract::PersonaId;

/// Wejście testowe: słowa po 250 ms z przerwami 60 ms, 800 ms ciszy przed i po.
pub fn utterance(text: &str, activity: AgentActivity) -> CmdInput {
    let mut tokens = Vec::new();
    let mut t = 1000;
    for word in text.split_whitespace() {
        tokens.push(Token::new(word, t, t + 250));
        t += 310;
    }
    let end = tokens.last().map_or(t, |w| w.end_ms);
    CmdInput {
        tokens,
        source: CmdSource::Final,
        activity,
        now_ms: end + 800,
        prev_speech_end_ms: Some(200),
        addressed: true,
    }
}

fn hit_of<R: CommandRecognizer>(
    r: &R,
    text: &str,
    activity: AgentActivity,
) -> Option<VoiceCommand> {
    r.recognize(&utterance(text, activity))
        .hit()
        .map(|h| h.command.clone())
}

/// Podstawowe komendy są rozpoznawane z pełną pewnością ≥ progu.
pub fn basic_commands<R: CommandRecognizer>(r: &R) {
    use AgentActivity::{Silent, Speaking};
    assert_eq!(hit_of(r, "stop", Speaking), Some(VoiceCommand::Stop));
    assert_eq!(hit_of(r, "czekaj", Speaking), Some(VoiceCommand::Wait));
    assert_eq!(
        hit_of(r, "głośniej", Speaking),
        Some(VoiceCommand::VolumeUp)
    );
    assert_eq!(
        hit_of(r, "stop wszystko", Speaking),
        Some(VoiceCommand::StopAll)
    );
    assert_eq!(
        hit_of(r, "przełącz na Deltę", Silent),
        Some(VoiceCommand::SwitchPersona {
            persona: PersonaId::delta()
        })
    );
    let decision = r.recognize(&utterance("stop", Speaking));
    let hit = decision
        .hit()
        .unwrap_or_else(|| panic!("brak trafienia: {decision:?}"));
    assert!(hit.confidence >= r.grammar().threshold && hit.confidence <= 1.0);
    assert_eq!(hit.at_ms, 1000);
}

/// Zdania ze słowami-komendami w innym znaczeniu nie są komendami.
pub fn sentences_are_not_commands<R: CommandRecognizer>(r: &R) {
    for text in [
        "pauza w szkole była długa",
        "stop-klatka w filmie",
        "nie no, dobrze",
        "przełącz na inny kanał",
        "",
    ] {
        assert_eq!(hit_of(r, text, AgentActivity::Speaking), None, "{text}");
    }
    assert_eq!(
        r.recognize(&utterance("", AgentActivity::Speaking)),
        CmdDecision::NoMatch
    );
}

/// Reguła „nie”: tylko samodzielnie, z pauzami, tylko w `Speaking`.
pub fn nie_rule<R: CommandRecognizer>(r: &R) {
    assert_eq!(
        hit_of(r, "nie", AgentActivity::Speaking),
        Some(VoiceCommand::No)
    );
    assert!(matches!(
        r.recognize(&utterance("nie", AgentActivity::Silent)),
        CmdDecision::Ignored {
            reason: IgnoreReason::NieOutsideSpeaking,
            ..
        }
    ));
    let mut glued = utterance("nie", AgentActivity::Speaking);
    glued.prev_speech_end_ms = Some(950);
    assert_eq!(r.recognize(&glued).hit(), None);
    let mut early = utterance("nie", AgentActivity::Speaking);
    early.now_ms = 1300;
    assert!(matches!(r.recognize(&early), CmdDecision::Pending { .. }));
}

/// Partial: trafienie dopiero po ciszy `settle_ms` po ostatnim słowie.
pub fn partial_waits_for_settle<R: CommandRecognizer>(r: &R) {
    let settle = r.grammar().settle_ms;
    let mut input = utterance("stop", AgentActivity::Speaking);
    input.source = CmdSource::Partial;
    input.now_ms = 1250;
    assert_eq!(
        r.recognize(&input),
        CmdDecision::Pending {
            recheck_at_ms: 1250 + settle
        }
    );
    input.now_ms = 1250 + settle;
    assert_eq!(
        r.recognize(&input).hit().map(|h| h.source),
        Some(CmdSource::Partial)
    );
}

/// Bez adresata działa tylko przerwanie mowy (stop w `Speaking`); reszta jest ignorowana.
pub fn addressing<R: CommandRecognizer>(r: &R) {
    let mut stop = utterance("stop", AgentActivity::Speaking);
    stop.addressed = false;
    assert!(r.recognize(&stop).hit().is_some());
    let mut louder = utterance("głośniej", AgentActivity::Silent);
    louder.addressed = false;
    assert!(matches!(
        r.recognize(&louder),
        CmdDecision::Ignored {
            reason: IgnoreReason::NotAddressed,
            ..
        }
    ));
}

/// Uruchamia cały zestaw.
pub fn run_all<R, F>(factory: F)
where
    R: CommandRecognizer,
    F: Fn() -> R,
{
    basic_commands(&factory());
    sentences_are_not_commands(&factory());
    nie_rule(&factory());
    partial_waits_for_settle(&factory());
    addressing(&factory());
}
