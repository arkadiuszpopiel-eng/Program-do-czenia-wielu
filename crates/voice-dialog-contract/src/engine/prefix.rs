//! „Usłyszany prefiks” (PLAN §6.5): (1) znaczniki słów z TTS, (2) alignment (te same znaczniki
//! ze źródłem `Alignment`), (3) liczenie odtworzonych próbek skorygowane o opóźnienie urządzenia →
//! przycięcie do granicy słowa/zdania + flaga `approximate`.

use crate::{ApproxTrim, HeardPrefix, MarkSource, PrefixSource, Utterance};

/// Największa granica słowa (indeks spacji lub 0) nie dalej niż `est` znaków.
fn word_boundary(text: &str, est: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    if est >= chars.len() {
        return chars.len();
    }
    (0..=est)
        .rev()
        .find(|i| *i == 0 || chars.get(*i).is_some_and(|c| c.is_whitespace()))
        .unwrap_or(0)
}

/// Prefiks usłyszany do bieżącej pozycji odtwarzania wypowiedzi.
pub fn heard_prefix(u: &Utterance, trim: ApproxTrim) -> HeardPrefix {
    let pos = u.played_ms;
    let mut chars = 0usize;
    let mut approximate = false;
    let mut source = if pos == 0 {
        PrefixSource::NothingPlayed
    } else {
        PrefixSource::SampleCount
    };
    for c in &u.chunks {
        let len = c.text.chars().count();
        if pos >= c.ms_start + c.audio_ms {
            chars = c.char_start + len;
            continue;
        }
        if pos <= c.ms_start {
            break;
        }
        let rel = pos - c.ms_start;
        if let Some(marks) = &c.marks {
            let end = marks
                .iter()
                .filter(|m| m.end_ms <= rel)
                .map(|m| m.char_end)
                .max()
                .unwrap_or(0);
            chars = c.char_start + end.min(len);
            source = match c.mark_source {
                Some(MarkSource::Alignment) => PrefixSource::Alignment,
                _ => PrefixSource::WordMarks,
            };
        } else {
            let len64 = u64::try_from(len).unwrap_or(u64::MAX);
            let est = usize::try_from(len64.saturating_mul(rel) / c.audio_ms.max(1)).unwrap_or(len);
            let within = match trim {
                ApproxTrim::Word => word_boundary(&c.text, est),
                ApproxTrim::Sentence => 0,
            };
            chars = c.char_start + within;
            approximate = true;
            source = PrefixSource::SampleCount;
        }
        break;
    }
    let text: String = u.full_text().chars().take(chars).collect();
    let text = text.trim_end().to_owned();
    HeardPrefix {
        utterance: u.id,
        chars: text.chars().count(),
        words: text.split_whitespace().count(),
        text,
        approximate,
        source,
    }
}

/// Reszta tekstu po prefiksie (bez wiodących spacji).
pub fn unsaid(u: &Utterance, heard: &HeardPrefix) -> String {
    u.full_text()
        .chars()
        .skip(heard.chars)
        .collect::<String>()
        .trim_start()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SpokenChunk, UtteranceId, WordMark};
    use personas_contract::PersonaId;

    fn utt(played_ms: u64, marks: bool) -> Utterance {
        let first = "Ala ma kota.";
        let m = |cs, ce, s, e| WordMark {
            char_start: cs,
            char_end: ce,
            start_ms: s,
            end_ms: e,
        };
        Utterance {
            id: UtteranceId(1),
            persona: PersonaId::alfa(),
            chunks: vec![
                SpokenChunk {
                    text: first.into(),
                    audio_ms: 1200,
                    char_start: 0,
                    ms_start: 0,
                    marks: marks
                        .then(|| vec![m(0, 3, 0, 300), m(4, 6, 400, 600), m(7, 12, 700, 1200)]),
                    mark_source: marks.then_some(MarkSource::Tts),
                },
                SpokenChunk {
                    text: "Kot ma Alę.".into(),
                    audio_ms: 1000,
                    char_start: 13,
                    ms_start: 1200,
                    marks: None,
                    mark_source: None,
                },
            ],
            played_ms,
            proactive: None,
        }
    }

    #[test]
    fn marks_give_word_accurate_prefix() {
        let h = heard_prefix(&utt(650, true), ApproxTrim::Word);
        assert_eq!(
            (h.text.as_str(), h.words, h.approximate, h.source),
            ("Ala ma", 2, false, PrefixSource::WordMarks)
        );
        assert_eq!(unsaid(&utt(650, true), &h), "kota. Kot ma Alę.");
    }

    #[test]
    fn samples_give_approximate_prefix_trimmed() {
        let h = heard_prefix(&utt(1700, true), ApproxTrim::Word);
        assert_eq!(
            (h.text.as_str(), h.approximate, h.source),
            ("Ala ma kota. Kot", true, PrefixSource::SampleCount)
        );
        let s = heard_prefix(&utt(1700, true), ApproxTrim::Sentence);
        assert_eq!(s.text, "Ala ma kota.");
        let none = heard_prefix(&utt(0, false), ApproxTrim::Word);
        assert_eq!((none.chars, none.source), (0, PrefixSource::NothingPlayed));
        let all = heard_prefix(&utt(5000, false), ApproxTrim::Word);
        assert_eq!(all.text, "Ala ma kota. Kot ma Alę.");
        assert!(!all.approximate);
    }
}
