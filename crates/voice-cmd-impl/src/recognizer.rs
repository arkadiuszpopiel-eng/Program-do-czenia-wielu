//! `GrammarRecognizer` — rozpoznawanie komend z gramatyki na transkrypcie partial/final.

use voice_cmd_contract::{
    AgentActivity, CmdDecision, CmdHit, CmdInput, CmdSource, CommandKind, CommandRecognizer,
    Grammar, IgnoreReason, Token, VoiceCommand, nie_verdict, split_tokens,
};

use crate::pattern::Compiled;

/// Wynik analizy wypowiedzi.
struct Parse {
    command: VoiceCommand,
    confidence: f32,
    first: usize,
    last: usize,
    named: bool,
}

/// Rozpoznawanie komend z gramatyki (deterministyczne, bez LLM).
#[derive(Debug, Clone)]
pub struct GrammarRecognizer {
    grammar: Grammar,
    compiled: Compiled,
}

impl Default for GrammarRecognizer {
    fn default() -> Self {
        Self::new(Grammar::default_pl_en())
    }
}

impl GrammarRecognizer {
    /// Rozpoznawanie z podaną gramatyką.
    pub fn new(grammar: Grammar) -> Self {
        let compiled = Compiled::new(&grammar);
        Self { grammar, compiled }
    }

    /// Wypowiedź = frazy komend + wypełniacze + imiona; inne słowo → `None` (zwykła wypowiedź).
    fn parse(&self, words: &[Token]) -> Option<Parse> {
        let texts: Vec<String> = words.iter().map(|w| w.text.clone()).collect();
        let mut i = 0;
        let mut found: Option<(VoiceCommand, f32, usize, usize)> = None;
        let mut named = false;
        while i < texts.len() {
            if let Some((kind, m)) = self.compiled.best_at(&texts, i) {
                let command = match kind {
                    CommandKind::SwitchPersona => VoiceCommand::SwitchPersona {
                        persona: m.persona.clone()?,
                    },
                    other => VoiceCommand::from_kind(other)?,
                };
                if found.is_none() {
                    found = Some((command, m.score, i, m.end - 1));
                } else if let Some(f) = found.as_mut() {
                    f.1 = f.1.min(m.score);
                }
                i = m.end;
                continue;
            }
            let word = texts.get(i)?;
            if self.compiled.is_filler(word) {
                i += 1;
                continue;
            }
            if self.compiled.address(word).is_some() {
                named = true;
                i += 1;
                continue;
            }
            return None;
        }
        let (command, score, first, last) = found?;
        let asr = words
            .iter()
            .map(|w| w.confidence)
            .fold(1.0_f32, f32::min)
            .clamp(0.0, 1.0);
        Some(Parse {
            command,
            confidence: (score * asr).clamp(0.0, 1.0),
            first,
            last,
            named,
        })
    }
}

impl CommandRecognizer for GrammarRecognizer {
    fn recognize(&self, input: &CmdInput) -> CmdDecision {
        let words = split_tokens(&input.tokens);
        if words.is_empty() {
            return CmdDecision::NoMatch;
        }
        if let Some(decision) = nie_verdict(input, &words, &self.grammar.nie) {
            return decision;
        }
        let Some(parse) = self.parse(&words) else {
            return CmdDecision::NoMatch;
        };
        let addressed = input.addressed || parse.named;
        if parse.confidence < self.grammar.threshold {
            return CmdDecision::Ignored {
                command: parse.command,
                reason: IgnoreReason::LowConfidence {
                    confidence: parse.confidence,
                },
            };
        }
        let barge_in = parse.command.is_barge_in() && input.activity != AgentActivity::Silent;
        if !addressed && !barge_in {
            return CmdDecision::Ignored {
                command: parse.command,
                reason: IgnoreReason::NotAddressed,
            };
        }
        let (Some(first), Some(last), Some(end)) =
            (words.get(parse.first), words.get(parse.last), words.last())
        else {
            return CmdDecision::NoMatch;
        };
        if input.source == CmdSource::Partial && input.now_ms < end.end_ms + self.grammar.settle_ms
        {
            return CmdDecision::Pending {
                recheck_at_ms: end.end_ms + self.grammar.settle_ms,
            };
        }
        let before = input
            .prev_speech_end_ms
            .map_or(u64::MAX, |p| first.start_ms.saturating_sub(p));
        let after = input.now_ms.saturating_sub(last.end_ms);
        let standalone = before >= self.grammar.nie.min_pause_before_ms
            && (after >= self.grammar.nie.min_pause_after_ms || parse.last + 1 == words.len());
        CmdDecision::Hit(CmdHit {
            command: parse.command,
            source: input.source,
            confidence: parse.confidence,
            at_ms: first.start_ms,
            standalone,
            addressed,
        })
    }

    fn grammar(&self) -> Grammar {
        self.grammar.clone()
    }
}
