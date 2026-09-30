//! Chunker strumienia tekstu dla TTS: zdanie po zdaniu, średnik, linia, przecinek w długim
//! zdaniu; nie tnie po skrótach („np. to”), w liczbach dziesiętnych ani wewnątrz bloku kodu.

use voice_persona_contract::{Boundary, Chunk, ChunkerCfg, SpeechChunker};

/// Skróty, po których kropka nigdy nie kończy zdania.
const NON_FINAL: &[&str] = &[
    "np", "m.in", "tzn", "tj", "tzw", "wg", "dr", "prof", "mgr", "inż", "ul", "al", "godz", "nr",
    "tel", "ok", "zob", "ds", "ww", "ew", "św", "ang", "pkt", "p", "św", "gen", "płk", "ks",
    "przyp", "vs", "e.g", "i.e", "mr", "mrs", "dr", "st",
];
/// Skróty, które mogą kończyć zdanie — tniemy tylko, gdy dalej jest wielka litera.
const MAYBE_FINAL: &[&str] = &[
    "itd", "itp", "etc", "jw", "cdn", "br", "r", "tys", "mln", "mld", "n.e", "p.n.e", "min", "sek",
];

const CLOSERS: &[char] = &['"', '”', '’', '\'', ')', ']', '»'];

/// Chunker zdaniowy (stanowy, deterministyczny).
#[derive(Debug, Clone)]
pub struct SentenceChunker {
    cfg: ChunkerCfg,
    buf: String,
    emitted: usize,
}

impl SentenceChunker {
    /// Nowy chunker z parametrami.
    pub fn new(cfg: ChunkerCfg) -> Self {
        Self {
            cfg,
            buf: String::new(),
            emitted: 0,
        }
    }

    fn limit(&self) -> usize {
        if self.emitted == 0 {
            self.cfg.first_max_chars
        } else {
            self.cfg.max_chars
        }
    }

    fn take(&mut self, end: usize, boundary: Boundary, out: &mut Vec<Chunk>) {
        let head: String = self.buf.drain(..end).collect();
        let trimmed_len = self.buf.len() - self.buf.trim_start().len();
        self.buf.drain(..trimmed_len);
        let text = head.trim();
        if !text.is_empty() {
            self.emitted += 1;
            out.push(Chunk {
                text: text.to_owned(),
                boundary,
            });
        }
    }

    fn drain_ready(&mut self, is_final: bool) -> Vec<Chunk> {
        let mut out = Vec::new();
        loop {
            let trimmed_len = self.buf.len() - self.buf.trim_start().len();
            self.buf.drain(..trimmed_len);
            match find_split(&self.buf, is_final, self.limit(), &self.cfg) {
                Some((end, boundary)) if end > 0 => self.take(end, boundary, &mut out),
                _ => break,
            }
        }
        if is_final {
            let len = self.buf.len();
            self.take(len, Boundary::End, &mut out);
        }
        out
    }
}

impl SpeechChunker for SentenceChunker {
    fn push(&mut self, delta: &str) -> Vec<Chunk> {
        self.buf.push_str(delta);
        self.drain_ready(false)
    }

    fn finish(&mut self) -> Vec<Chunk> {
        self.drain_ready(true)
    }
}

/// Token przed kropką: litery i kropki (np. „m.in”, „np”), małymi literami.
fn token_before(text: &str, dot: usize) -> String {
    let head = &text[..dot];
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_alphabetic() || *c == '.'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    head[start..].trim_start_matches('.').to_lowercase()
}

enum Next {
    /// Brak znaku (koniec bufora).
    Unknown,
    /// Koniec tekstu przy `finish`.
    End,
    /// Następny znak po terminatorze i zamykających cudzysłowach.
    Char(char, usize),
}

fn next_after(text: &str, from: usize, is_final: bool) -> Next {
    let mut idx = from;
    for c in text[from..].chars() {
        if CLOSERS.contains(&c) {
            idx += c.len_utf8();
            continue;
        }
        return Next::Char(c, idx);
    }
    if is_final { Next::End } else { Next::Unknown }
}

/// Pierwszy nie-biały znak od `from`.
fn next_visible(text: &str, from: usize) -> Option<char> {
    text[from..].chars().find(|c| !c.is_whitespace())
}

/// Ocena terminatora na pozycji `pos` (znak `c`): `Some(koniec)` = tniemy, `None` = nie, `Err` = czekamy.
fn judge(text: &str, pos: usize, c: char, is_final: bool) -> Result<Option<(usize, Boundary)>, ()> {
    let run_end = pos
        + text[pos..]
            .chars()
            .take_while(|ch| matches!(ch, '.' | '!' | '?' | '…' | ';'))
            .map(char::len_utf8)
            .sum::<usize>();
    let (end, next) = match next_after(text, run_end, is_final) {
        Next::Unknown => return Err(()),
        Next::End => return Ok(Some((text.len(), boundary_of(c)))),
        Next::Char(ch, idx) => (idx, ch),
    };
    if !next.is_whitespace() {
        return Ok(None);
    }
    let run = &text[pos..run_end];
    let ambiguous = run == "." || run == "…" || run == "...";
    if !ambiguous || c == ';' {
        return Ok(Some((end, boundary_of(c))));
    }
    let upcoming = next_visible(text, end);
    if upcoming.is_none() && !is_final {
        return Err(());
    }
    let starts_new = upcoming.is_none_or(|u| {
        u.is_uppercase() || u.is_ascii_digit() || u == '\n' || matches!(u, '-' | '*' | '„' | '"')
    });
    let newline = text[end..]
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .any(|ch| ch == '\n');
    let before = token_before(text, pos);
    let prev_char = text[..pos].chars().last();
    let decision = if run != "." {
        starts_new || newline
    } else if NON_FINAL.contains(&before.as_str()) {
        newline
    } else if MAYBE_FINAL.contains(&before.as_str())
        || prev_char.is_some_and(|p| p.is_ascii_digit())
    {
        starts_new || newline
    } else if before.chars().count() == 1 && prev_char.is_some_and(char::is_uppercase) {
        newline
    } else {
        true
    };
    Ok(decision.then_some((end, Boundary::Sentence)))
}

fn boundary_of(c: char) -> Boundary {
    if c == ';' {
        Boundary::Semicolon
    } else {
        Boundary::Sentence
    }
}

/// Szuka pierwszego miejsca podziału w buforze.
fn find_split(
    text: &str,
    is_final: bool,
    limit: usize,
    cfg: &ChunkerCfg,
) -> Option<(usize, Boundary)> {
    let mut in_fence = false;
    let mut chars_seen = 0usize;
    let mut last_clause: Option<usize> = None;
    let mut last_space: Option<usize> = None;
    let mut iter = text.char_indices().peekable();
    while let Some((pos, c)) = iter.next() {
        chars_seen += 1;
        if text[pos..].starts_with("```") {
            if in_fence {
                let close = pos + 3;
                let line_end = text[close..]
                    .find('\n')
                    .map_or(text.len(), |n| close + n + 1);
                if line_end == text.len() && !is_final && !text[close..].contains('\n') {
                    return None;
                }
                return Some((line_end, Boundary::Code));
            }
            in_fence = true;
            iter.next();
            iter.next();
            continue;
        }
        if in_fence {
            continue;
        }
        match c {
            '\n' if !text[..pos].trim().is_empty() => return Some((pos + 1, Boundary::Line)),
            '.' | '!' | '?' | '…' | ';' => match judge(text, pos, c, is_final) {
                Err(()) => return None,
                Ok(Some(hit)) => return Some(hit),
                Ok(None) => {}
            },
            ',' | ':' | '–' | '—' => {
                let spaced = text[pos + c.len_utf8()..].starts_with(char::is_whitespace);
                if spaced && chars_seen >= cfg.min_clause_chars && chars_seen <= limit {
                    last_clause = Some(pos + c.len_utf8());
                }
            }
            _ if c.is_whitespace() && chars_seen <= cfg.hard_max_chars => last_space = Some(pos),
            _ => {}
        }
        if chars_seen > limit
            && let Some(end) = last_clause
        {
            return Some((end, Boundary::Clause));
        }
        if chars_seen > cfg.hard_max_chars
            && let Some(end) = last_space
        {
            return Some((end, Boundary::Forced));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(text: &str) -> Vec<String> {
        let mut c = SentenceChunker::new(ChunkerCfg::default());
        let mut out: Vec<String> = c.push(text).into_iter().map(|c| c.text).collect();
        out.extend(c.finish().into_iter().map(|c| c.text));
        out
    }

    #[test]
    fn splits_sentences_but_not_abbreviations_or_decimals() {
        assert_eq!(
            chunks("Np. to działa. Kosztuje 3.5 zł, czyli 12,50 zł! A dalej?"),
            vec![
                "Np. to działa.",
                "Kosztuje 3.5 zł, czyli 12,50 zł!",
                "A dalej?"
            ]
        );
        assert_eq!(
            chunks("Zrobiłam m.in. testy itd. Potem koniec."),
            vec!["Zrobiłam m.in. testy itd.", "Potem koniec."]
        );
        assert_eq!(
            chunks("J. Kowalski przyszedł. Dobrze."),
            vec!["J. Kowalski przyszedł.", "Dobrze."]
        );
        assert_eq!(chunks("Pierwsze; drugie"), vec!["Pierwsze;", "drugie"]);
        assert_eq!(chunks("„Tak.” Potem nie."), vec!["„Tak.”", "Potem nie."]);
    }

    #[test]
    fn long_sentence_splits_on_comma_then_space() {
        let long = format!("{}, {}.", "a".repeat(50), "b ".repeat(80));
        let out = chunks(&long);
        assert!(out.len() >= 2, "{out:?}");
        assert!(out[0].ends_with(','));
        let no_comma = "słowo ".repeat(80);
        assert!(chunks(&no_comma).iter().all(|c| c.chars().count() <= 300));
    }

    #[test]
    fn code_fence_is_one_chunk() {
        let out = chunks("Oto kod:\n```rust\nfn a() {}\n\nfn b() {}\n```\nKoniec.");
        assert_eq!(
            out,
            vec![
                "Oto kod:",
                "```rust\nfn a() {}\n\nfn b() {}\n```",
                "Koniec."
            ]
        );
    }

    #[test]
    fn waits_for_lookahead_in_stream() {
        let mut c = SentenceChunker::new(ChunkerCfg::default());
        assert!(c.push("To jest np.").is_empty());
        assert!(c.push(" to").is_empty());
        assert!(c.push(" koniec.").is_empty());
        let got = c.push(" Nowe");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "To jest np. to koniec.");
        assert_eq!(c.finish()[0].boundary, Boundary::End);
    }
}
