//! Wspólne reguły tekstowe (dzielone przez `-impl` i `-fake`, żeby się nie rozjechały).

use crate::{
    AgentActivity, CmdDecision, CmdHit, CmdInput, IgnoreReason, NieRule, Token, VoiceCommand,
};

/// Składanie tekstu do porównań: małe litery, bez polskich znaków, bez interpunkcji na brzegach.
pub fn fold(text: &str) -> String {
    let trimmed = text.trim_matches(|c: char| !c.is_alphanumeric());
    trimmed
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            other => other,
        })
        .collect()
}

/// Dzieli tokeny na słowa (token z ASR może zawierać kilka słów — czas dzielony proporcjonalnie)
/// i składa je `fold`; puste po złożeniu są pomijane.
pub fn split_tokens(tokens: &[Token]) -> Vec<Token> {
    let mut out = Vec::new();
    for t in tokens {
        let words: Vec<&str> = t.text.split_whitespace().collect();
        let n = u64::try_from(words.len()).unwrap_or(1).max(1);
        let span = t.end_ms.saturating_sub(t.start_ms);
        for (i, w) in words.iter().enumerate() {
            let folded = fold(w);
            if folded.is_empty() {
                continue;
            }
            let i = u64::try_from(i).unwrap_or(0);
            out.push(Token {
                text: folded,
                start_ms: t.start_ms + span * i / n,
                end_ms: t.start_ms + span * (i + 1) / n,
                confidence: t.confidence,
            });
        }
    }
    out
}

/// Reguła samodzielnego „nie” (PLAN §6.5): `None`, gdy wypowiedź to nie same „nie”.
pub fn nie_verdict(input: &CmdInput, words: &[Token], rule: &NieRule) -> Option<CmdDecision> {
    if words.is_empty()
        || !words
            .iter()
            .all(|w| rule.words.iter().any(|n| fold(n) == w.text))
    {
        return None;
    }
    let first = words.first()?;
    let last = words.last()?;
    let pause_before = input
        .prev_speech_end_ms
        .map_or(u64::MAX, |end| first.start_ms.saturating_sub(end));
    if pause_before < rule.min_pause_before_ms {
        return Some(CmdDecision::NoMatch);
    }
    if input.activity != AgentActivity::Speaking {
        return Some(CmdDecision::Ignored {
            command: VoiceCommand::No,
            reason: IgnoreReason::NieOutsideSpeaking,
        });
    }
    let pause_after = input.now_ms.saturating_sub(last.end_ms);
    if pause_after < rule.min_pause_after_ms {
        return Some(CmdDecision::Pending {
            recheck_at_ms: last.end_ms + rule.min_pause_after_ms,
        });
    }
    let confidence = words
        .iter()
        .map(|w| w.confidence)
        .fold(1.0_f32, f32::min)
        .clamp(0.0, 1.0);
    Some(CmdDecision::Hit(CmdHit {
        command: VoiceCommand::No,
        source: input.source,
        confidence,
        at_ms: first.start_ms,
        standalone: true,
        addressed: input.addressed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CmdSource;

    fn input(
        tokens: Vec<Token>,
        activity: AgentActivity,
        now_ms: u64,
        prev: Option<u64>,
    ) -> CmdInput {
        CmdInput {
            tokens,
            source: CmdSource::Final,
            activity,
            now_ms,
            prev_speech_end_ms: prev,
            addressed: true,
        }
    }

    #[test]
    fn fold_strips_diacritics_and_punctuation() {
        assert_eq!(fold("„Głośniej!”"), "glosniej");
        assert_eq!(fold("Stop-klatka,"), "stop-klatka");
        assert_eq!(fold("..."), "");
    }

    #[test]
    fn split_distributes_time() {
        let words = split_tokens(&[Token::new("mów dalej", 0, 400), Token::new(",", 400, 410)]);
        assert_eq!(words.len(), 2);
        assert_eq!(
            (words[1].text.as_str(), words[1].start_ms, words[1].end_ms),
            ("dalej", 200, 400)
        );
    }

    #[test]
    fn nie_rule_requires_pauses_and_speaking() {
        let rule = NieRule::default();
        let tok = || vec![Token::new("nie", 1000, 1200)];
        let words = |i: &CmdInput| split_tokens(&i.tokens);
        let ok = input(tok(), AgentActivity::Speaking, 1600, Some(200));
        assert!(matches!(
            nie_verdict(&ok, &words(&ok), &rule),
            Some(CmdDecision::Hit(_))
        ));
        let early = input(tok(), AgentActivity::Speaking, 1300, Some(200));
        assert_eq!(
            nie_verdict(&early, &words(&early), &rule),
            Some(CmdDecision::Pending {
                recheck_at_ms: 1500
            })
        );
        let glued = input(tok(), AgentActivity::Speaking, 1600, Some(950));
        assert_eq!(
            nie_verdict(&glued, &words(&glued), &rule),
            Some(CmdDecision::NoMatch)
        );
        let idle = input(tok(), AgentActivity::Silent, 1600, None);
        assert!(matches!(
            nie_verdict(&idle, &words(&idle), &rule),
            Some(CmdDecision::Ignored { .. })
        ));
        let more = input(
            vec![Token::new("nie no", 1000, 1300)],
            AgentActivity::Speaking,
            2000,
            None,
        );
        assert_eq!(nie_verdict(&more, &words(&more), &rule), None);
    }
}
