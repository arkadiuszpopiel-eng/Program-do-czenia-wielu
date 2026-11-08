//! Tokens plus raw source ranges: quoted data must not hide executable UNC expressions.
use std::ops::Range;

pub(crate) struct Lexed {
    pub words: Vec<String>,
    spans: Vec<Range<usize>>,
}

impl Lexed {
    pub fn literal_regex(&self, source: &str, index: usize) -> bool {
        if index < 2 {
            return false;
        }
        let (Some(left), Some(op), Some(arg)) = (
            self.spans.get(index - 2),
            self.spans.get(index - 1),
            self.spans.get(index),
        ) else {
            return false;
        };
        if !matches!(
            self.words[index - 1].to_ascii_lowercase().as_str(),
            "-match"
                | "-notmatch"
                | "-imatch"
                | "-inotmatch"
                | "-cmatch"
                | "-cnotmatch"
                | "-replace"
                | "-ireplace"
                | "-creplace"
        ) {
            return false;
        }
        // Only a variable expression at a statement/group boundary, not a native
        // command argument such as findstr.exe $x -match '\\server\share'.
        if !source[left.clone()].starts_with('$') {
            return false;
        }
        let prefix = source[..left.start].trim_end();
        if !prefix.is_empty()
            && !prefix
                .chars()
                .last()
                .is_some_and(|c| matches!(c, '{' | ';' | '('))
        {
            return false;
        }
        if !source[left.end..op.start].chars().all(char::is_whitespace)
            || !source[op.end..arg.start].chars().all(char::is_whitespace)
        {
            return false;
        }
        // '&', parentheses and other expression syntax in the gap are not literal
        // data. Single quotes allow regex '$' anchors; double quotes may interpolate.
        let raw = &source[arg.clone()];
        raw.len() >= 2
            && matches!(raw.as_bytes().first(), Some(b'\'' | b'"'))
            && raw.as_bytes().first() == raw.as_bytes().last()
            && (!raw.starts_with('"') || !raw.contains('$'))
    }
}

pub(crate) fn scan(command: &str) -> Lexed {
    let mut result = Lexed {
        words: Vec::new(),
        spans: Vec::new(),
    };
    let mut cur = String::new();
    let mut start = None;
    let mut quote = None;
    for (index, c) in command.char_indices() {
        match (c, quote) {
            ('^' | '\u{60}', _) => {
                start.get_or_insert(index);
            }
            ('"' | '\'', None) => {
                start.get_or_insert(index);
                quote = Some(c);
            }
            (q, Some(open)) if q == open => quote = None,
            (c, None)
                if c.is_whitespace()
                    || matches!(c, ';' | '&' | '|' | '(' | ')' | ',' | '{' | '}' | '<' | '>') =>
            {
                if !cur.is_empty() {
                    result.words.push(std::mem::take(&mut cur));
                    result.spans.push(start.unwrap_or(index)..index);
                }
                start = None;
            }
            (c, _) => {
                start.get_or_insert(index);
                cur.push(c);
            }
        }
    }
    if !cur.is_empty() {
        result.words.push(cur);
        result
            .spans
            .push(start.unwrap_or(command.len())..command.len());
    }
    result
}
