//! Podział mówione/ekranowe: kod i tabele na ekran, markdown zdjęty z kanału mówionego,
//! znaczniki stylu zamienione na znaczniki prywatne (U+E000…) do przypisania zdaniom.

use voice_persona_contract::{CODE_ON_SCREEN, Emotion, Energy, StyleTags, Tempo};

/// Tekst mówiony w miejsce tabeli.
pub const TABLE_ON_SCREEN: &str = "(tabela na ekranie)";

const MARK_BASE: u32 = 0xE000;

/// Znacznik stylu wyciągnięty z tekstu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StyleMark {
    Emotion(Emotion),
    Tempo(Tempo),
    Energy(Energy),
    Unknown(String),
}

/// Wynik podziału: tekst kanału mówionego (z markerami), bloki ekranowe i znaczniki.
pub(crate) struct Split {
    pub spoken: String,
    pub screen: Vec<String>,
    pub marks: Vec<StyleMark>,
}

fn parse_tag(inner: &str) -> Option<StyleMark> {
    let (key, value) = inner.split_once(':')?;
    let key = key.trim().to_lowercase();
    let mark = match key.as_str() {
        "emocja" | "emotion" => Emotion::from_tag(value).map(StyleMark::Emotion),
        "tempo" => Tempo::from_tag(value).map(StyleMark::Tempo),
        "energia" | "energy" => Energy::from_tag(value).map(StyleMark::Energy),
        _ => return None,
    };
    Some(mark.unwrap_or_else(|| StyleMark::Unknown(format!("{key}:{}", value.trim()))))
}

/// Usuwa formatowanie markdown z linii, zamienia znaczniki stylu na markery.
fn strip_inline(line: &str, marks: &mut Vec<StyleMark>) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        if c == '!'
            && rest.starts_with("![")
            && let Some(end) = rest.find(')').filter(|e| rest[..*e].contains("]("))
        {
            rest = &rest[end + 1..];
            continue;
        }
        if c == '['
            && let Some(close) = rest.find(']')
        {
            let inner = &rest[1..close];
            let after = &rest[close + 1..];
            if let Some(mark) = parse_tag(inner) {
                let code = MARK_BASE + u32::try_from(marks.len()).unwrap_or(0);
                marks.push(mark);
                out.extend(char::from_u32(code));
                rest = after;
                continue;
            }
            if after.starts_with('(')
                && let Some(end) = after.find(')')
            {
                out.push_str(inner);
                rest = &after[end + 1..];
                continue;
            }
            if !inner.is_empty() && inner.chars().all(|ch| ch.is_ascii_digit()) {
                rest = after;
                continue;
            }
        }
        if c == '<'
            && let Some(end) = rest.find('>').filter(|e| {
                let tag = &rest[1..*e];
                !tag.is_empty()
                    && tag
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '/')
            })
        {
            rest = &rest[end + 1..];
            continue;
        }
        if rest.starts_with("**") || rest.starts_with("__") || rest.starts_with("~~") {
            rest = &rest[2..];
            continue;
        }
        let prev = out.chars().last();
        let next = rest[c.len_utf8()..].chars().next();
        let emphasis = (c == '*' || c == '_')
            && (prev.is_none_or(|p| !p.is_alphanumeric())
                || next.is_none_or(|n| !n.is_alphanumeric()));
        if emphasis {
            rest = &rest[1..];
            continue;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Zdejmuje znacznik bloku (nagłówek, lista, cytat) z początku linii.
fn strip_block_prefix(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '))
        && t.chars().filter(|c| !c.is_whitespace()).count() >= 3
    {
        return None;
    }
    let t = t
        .trim_start_matches('#')
        .trim_start_matches('>')
        .trim_start();
    for bullet in ["- ", "* ", "+ ", "• "] {
        if let Some(rest) = t.strip_prefix(bullet) {
            return Some(rest);
        }
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let after = &t[digits..];
        if let Some(rest) = after
            .strip_prefix(". ")
            .or_else(|| after.strip_prefix(") "))
        {
            return Some(rest);
        }
    }
    Some(t)
}

/// Dzieli odpowiedź asystentki na kanał mówiony i ekranowy.
pub(crate) fn split(text: &str) -> Split {
    let mut spoken_lines: Vec<String> = Vec::new();
    let mut screen = Vec::new();
    let mut marks = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            let mut block = vec![line.to_owned()];
            for inner in lines.by_ref() {
                block.push(inner.to_owned());
                if inner.trim_start().starts_with("```") {
                    break;
                }
            }
            screen.push(block.join("\n"));
            spoken_lines.push(CODE_ON_SCREEN.to_owned());
            continue;
        }
        if trimmed.starts_with('|') {
            let mut block = vec![line.to_owned()];
            while let Some(next) = lines.next_if(|l| l.trim_start().starts_with('|')) {
                block.push(next.to_owned());
            }
            screen.push(block.join("\n"));
            spoken_lines.push(TABLE_ON_SCREEN.to_owned());
            continue;
        }
        if let Some(content) = strip_block_prefix(line) {
            let clean = strip_inline(content, &mut marks);
            if !clean.trim().is_empty() {
                spoken_lines.push(clean.trim().to_owned());
            }
        }
    }
    Split {
        spoken: spoken_lines.join("\n"),
        screen,
        marks,
    }
}

/// Wyciąga markery z tekstu zdania: stosuje je do `tags`, zwraca tekst bez markerów.
pub(crate) fn apply_marks(
    text: &str,
    marks: &[StyleMark],
    tags: &mut StyleTags,
    unknown: &mut Vec<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let code = u32::from(c);
        let idx = code
            .checked_sub(MARK_BASE)
            .and_then(|i| usize::try_from(i).ok());
        match idx.and_then(|i| marks.get(i)) {
            Some(StyleMark::Emotion(e)) => tags.emotion = Some(*e),
            Some(StyleMark::Tempo(t)) => tags.tempo = Some(*t),
            Some(StyleMark::Energy(e)) => tags.energy = Some(*e),
            Some(StyleMark::Unknown(s)) => unknown.push(s.clone()),
            None if (0xE000..=0xF8FF).contains(&code) => {}
            None => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markdown_and_moves_code_to_screen() {
        let s = split(
            "# Tytuł\n- **pierwszy** punkt\n1. drugi [link](http://x.pl) ![img](a.png)\n---\n```\nkod\n```\n| a |\n| 1 |\n> cytat [1]",
        );
        assert_eq!(s.screen.len(), 2);
        assert_eq!(
            s.spoken,
            format!(
                "Tytuł\npierwszy punkt\ndrugi link\n{CODE_ON_SCREEN}\n{TABLE_ON_SCREEN}\ncytat"
            )
        );
    }

    #[test]
    fn style_tags_become_marks() {
        let s = split("[emocja:radość] Hej [tempo:turbo] snake_case i *ważne*.");
        assert_eq!(s.marks.len(), 2);
        let mut tags = StyleTags::default();
        let mut unknown = Vec::new();
        let text = apply_marks(&s.spoken, &s.marks, &mut tags, &mut unknown);
        assert_eq!(text.trim(), "Hej  snake_case i ważne.");
        assert_eq!(tags.emotion, Some(Emotion::Joy));
        assert_eq!(unknown, vec!["tempo:turbo".to_owned()]);
    }
}
