//! Właściwości rozpoznawania (proptest, stałe ziarno).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use voice_cmd_contract::contract_tests::utterance;
use voice_cmd_contract::{AgentActivity, CmdDecision, CommandRecognizer, VoiceCommand};
use voice_cmd_impl::GrammarRecognizer;

const COMMANDS: &[(&str, VoiceCommand)] = &[
    ("stop", VoiceCommand::Stop),
    ("czekaj", VoiceCommand::Wait),
    ("pauza", VoiceCommand::Pause),
    ("kontynuuj", VoiceCommand::Resume),
    ("powtórz", VoiceCommand::Repeat),
    ("anuluj", VoiceCommand::Cancel),
    ("głośniej", VoiceCommand::VolumeUp),
    ("ciszej", VoiceCommand::VolumeDown),
    ("wycisz mikrofon", VoiceCommand::MuteMic),
    ("nie przeszkadzać", VoiceCommand::DoNotDisturb),
    ("stop wszystko", VoiceCommand::StopAll),
];
const FILLERS: &[&str] = &["proszę", "dobra", "okej", "hej", "już", "yyy"];
const CONTENT: &[&str] = &[
    "w",
    "szkole",
    "filmie",
    "kanał",
    "raport",
    "jutro",
    "kawa",
    "samochód",
    "the",
    "car",
    "pogoda",
    "mnie",
    "długa",
    "radio",
    "o",
    "to",
    "chodzi",
    "na",
    "przy",
    "wejściu",
];

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, failure_persistence: None, rng_seed: proptest::test_runner::RngSeed::Fixed(0xC0DE), ..ProptestConfig::default() })]

    /// Komenda otoczona wypełniaczami pozostaje tą samą komendą.
    #[test]
    fn fillers_do_not_change_command(cmd in 0..COMMANDS.len(), pre in prop::collection::vec(0..FILLERS.len(), 0..3), post in prop::collection::vec(0..FILLERS.len(), 0..3)) {
        let mut words: Vec<&str> = pre.iter().map(|i| FILLERS[*i]).collect();
        words.push(COMMANDS[cmd].0);
        words.extend(post.iter().map(|i| FILLERS[*i]));
        let r = GrammarRecognizer::default();
        let got = r.recognize(&utterance(&words.join(" "), AgentActivity::Speaking));
        prop_assert_eq!(got.hit().map(|h| h.command.clone()), Some(COMMANDS[cmd].1.clone()));
    }

    /// Dowolne słowo spoza gramatyki w wypowiedzi → to nie jest komenda.
    #[test]
    fn content_word_blocks_command(cmd in 0..COMMANDS.len(), content in prop::collection::vec(0..CONTENT.len(), 1..4), before in any::<bool>()) {
        let c: Vec<&str> = content.iter().map(|i| CONTENT[*i]).collect();
        let text = if before { format!("{} {}", c.join(" "), COMMANDS[cmd].0) } else { format!("{} {}", COMMANDS[cmd].0, c.join(" ")) };
        let r = GrammarRecognizer::default();
        let decision = r.recognize(&utterance(&text, AgentActivity::Speaking));
        prop_assert_eq!(decision.hit(), None);
    }

    /// Wynik jest deterministyczny, a pewność w [0, 1].
    #[test]
    fn deterministic_and_bounded(words in prop::collection::vec("[a-ząćęłńóśźż]{1,9}", 0..6), speaking in any::<bool>()) {
        let activity = if speaking { AgentActivity::Speaking } else { AgentActivity::Silent };
        let input = utterance(&words.join(" "), activity);
        let r = GrammarRecognizer::default();
        let a = r.recognize(&input);
        prop_assert_eq!(&a, &r.recognize(&input));
        if let CmdDecision::Hit(hit) = a {
            prop_assert!((0.0..=1.0).contains(&hit.confidence));
        }
    }
}
