//! Atrapa modułu `voice-cmd` (SPEC „Fake”): trafienia z adnotacji (`script`) oraz dokładne
//! dopasowanie fraz gramatyki (bez odległości edycyjnej), z tą samą regułą „nie”, adresowaniem
//! i progiem `settle_ms` co implementacja. Rejestruje wszystkie wejścia.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use voice_cmd_contract::{
    AgentActivity, CmdDecision, CmdHit, CmdInput, CmdSource, CommandKind, CommandRecognizer,
    Grammar, IgnoreReason, VoiceCommand, fold, nie_verdict, split_tokens,
};

#[derive(Debug, Default)]
struct State {
    script: BTreeMap<String, CmdDecision>,
    calls: Vec<CmdInput>,
}

/// Deterministyczna atrapa rozpoznawania komend.
#[derive(Debug)]
pub struct FakeRecognizer {
    grammar: Grammar,
    phrases: BTreeMap<String, VoiceCommand>,
    state: Mutex<State>,
}

impl Default for FakeRecognizer {
    fn default() -> Self {
        Self::new(Grammar::default_pl_en())
    }
}

fn expand(
    elems: &[&str],
    prefix: &mut Vec<String>,
    out: &mut Vec<Vec<String>>,
    persona_forms: &[String],
) {
    let Some((first, rest)) = elems.split_first() else {
        out.push(prefix.clone());
        return;
    };
    if *first == "{persona}" {
        for form in persona_forms {
            prefix.push(format!("{{{form}}}"));
            expand(rest, prefix, out, persona_forms);
            prefix.pop();
        }
        return;
    }
    let optional = first.starts_with('[') && first.ends_with(']');
    for alt in first
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split('|')
    {
        prefix.push(fold(alt));
        expand(rest, prefix, out, persona_forms);
        prefix.pop();
    }
    if optional {
        expand(rest, prefix, out, persona_forms);
    }
}

impl FakeRecognizer {
    /// Atrapa z gramatyką (frazy rozwijane do dokładnych ciągów słów).
    pub fn new(grammar: Grammar) -> Self {
        let mut phrases = BTreeMap::new();
        for rule in &grammar.rules {
            for phrase in &rule.phrases {
                let elems: Vec<&str> = phrase.split_whitespace().collect();
                for p in &grammar.personas {
                    let forms: Vec<String> = p.forms.iter().map(|f| fold(f)).collect();
                    let mut expanded = Vec::new();
                    expand(&elems, &mut Vec::new(), &mut expanded, &forms);
                    for words in expanded {
                        let key = words
                            .iter()
                            .map(|w| w.trim_matches(['{', '}']))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let command = match rule.command {
                            CommandKind::SwitchPersona
                                if words.iter().any(|w| w.starts_with('{')) =>
                            {
                                VoiceCommand::SwitchPersona {
                                    persona: p.persona.clone(),
                                }
                            }
                            CommandKind::SwitchPersona => continue,
                            other => match VoiceCommand::from_kind(other) {
                                Some(c) => c,
                                None => continue,
                            },
                        };
                        phrases.entry(key).or_insert(command);
                    }
                }
            }
        }
        Self {
            grammar,
            phrases,
            state: Mutex::new(State::default()),
        }
    }

    /// Adnotacja: dla wypowiedzi (po `fold`, słowa rozdzielone spacją) zwróć tę decyzję.
    pub fn script(&self, text: &str, decision: CmdDecision) {
        let key = text
            .split_whitespace()
            .map(fold)
            .collect::<Vec<_>>()
            .join(" ");
        self.lock().script.insert(key, decision);
    }

    /// Wszystkie wejścia przekazane do `recognize`.
    pub fn calls(&self) -> Vec<CmdInput> {
        self.lock().calls.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn is_address(&self, word: &str) -> bool {
        self.grammar
            .personas
            .iter()
            .any(|p| p.address_forms.iter().any(|f| fold(f) == word))
    }
}

impl CommandRecognizer for FakeRecognizer {
    fn recognize(&self, input: &CmdInput) -> CmdDecision {
        let words = split_tokens(&input.tokens);
        let key = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        {
            let mut st = self.lock();
            st.calls.push(input.clone());
            if let Some(decision) = st.script.get(&key) {
                return decision.clone();
            }
        }
        if words.is_empty() {
            return CmdDecision::NoMatch;
        }
        if let Some(decision) = nie_verdict(input, &words, &self.grammar.nie) {
            return decision;
        }
        let fillers: Vec<String> = self.grammar.fillers.iter().map(|f| fold(f)).collect();
        let named = words.iter().any(|w| self.is_address(&w.text));
        let core: Vec<_> = words
            .iter()
            .filter(|w| !fillers.contains(&w.text))
            .collect();
        let core_key = core
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let (command, core) = match self.phrases.get(&core_key) {
            Some(c) => (c.clone(), core),
            None => {
                let rest: Vec<_> = core
                    .into_iter()
                    .filter(|w| !self.is_address(&w.text))
                    .collect();
                let rest_key = rest
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                match self.phrases.get(&rest_key) {
                    Some(c) => (c.clone(), rest),
                    None => return CmdDecision::NoMatch,
                }
            }
        };
        let (Some(first), Some(last)) = (core.first(), words.last()) else {
            return CmdDecision::NoMatch;
        };
        let addressed = input.addressed || named;
        let barge_in = command.is_barge_in() && input.activity != AgentActivity::Silent;
        if !addressed && !barge_in {
            return CmdDecision::Ignored {
                command,
                reason: IgnoreReason::NotAddressed,
            };
        }
        if input.source == CmdSource::Partial && input.now_ms < last.end_ms + self.grammar.settle_ms
        {
            return CmdDecision::Pending {
                recheck_at_ms: last.end_ms + self.grammar.settle_ms,
            };
        }
        let before = input
            .prev_speech_end_ms
            .map_or(u64::MAX, |p| first.start_ms.saturating_sub(p));
        CmdDecision::Hit(CmdHit {
            command,
            source: input.source,
            confidence: 1.0,
            at_ms: first.start_ms,
            standalone: before >= self.grammar.nie.min_pause_before_ms,
            addressed,
        })
    }

    fn grammar(&self) -> Grammar {
        self.grammar.clone()
    }
}
