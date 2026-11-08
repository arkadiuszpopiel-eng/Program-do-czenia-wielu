//! Tokeny dodane (`added_tokens`, np. `<s>`, `<mask>`): wycinane z tekstu przed normalizacją
//! (`normalized = false`) albo po niej (`normalized = true`), dopasowanie najdłuższe od lewej,
//! `lstrip`/`rstrip` pochłaniają sąsiednie białe znaki — jak `AddedVocabulary` w HF.

use serde_json::Value;

use crate::error::EmbedError;

/// Token dodany.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedToken {
    /// Identyfikator.
    pub id: u32,
    /// Treść.
    pub content: String,
    /// Dopasowanie w tekście znormalizowanym.
    pub normalized: bool,
    /// Pochłania białe znaki z lewej.
    pub lstrip: bool,
    /// Pochłania białe znaki z prawej.
    pub rstrip: bool,
}

/// Fragment tekstu: zwykły tekst albo identyfikator tokenu dodanego.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Tekst (z przesunięciem początku w tekście źródłowym).
    Text(String, usize),
    /// Token dodany.
    Token(u32),
}

/// Parsuje `added_tokens` (brak pola → pusta lista). `single_word = true` nie jest obsługiwane.
pub fn parse(v: Option<&Value>) -> Result<Vec<AddedToken>, EmbedError> {
    let Some(items) = v.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let flag = |item: &Value, key: &str| item.get(key).and_then(Value::as_bool).unwrap_or(false);
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let id = item
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|u| u32::try_from(u).ok());
        let content = item.get("content").and_then(Value::as_str);
        let (Some(id), Some(content)) = (id, content) else {
            return Err(EmbedError::Tokenizer(
                "added_tokens: brak `id`/`content`".into(),
            ));
        };
        if flag(item, "single_word") || content.is_empty() {
            return Err(EmbedError::Tokenizer(format!(
                "added_tokens: `{content}` (single_word/pusty) nie jest obsługiwany"
            )));
        }
        out.push(AddedToken {
            id,
            content: content.to_owned(),
            normalized: item
                .get("normalized")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            lstrip: flag(item, "lstrip"),
            rstrip: flag(item, "rstrip"),
        });
    }
    Ok(out)
}

/// Dzieli tekst na tokeny dodane (z listy `tokens`) i resztę; `base` = przesunięcie tekstu.
pub fn split(text: &str, base: usize, tokens: &[&AddedToken]) -> Vec<Segment> {
    let mut out = Vec::new();
    if tokens.is_empty() {
        if !text.is_empty() {
            out.push(Segment::Text(text.to_owned(), base));
        }
        return out;
    }
    let mut last = 0;
    let mut pos = 0;
    while pos < text.len() {
        let best = tokens
            .iter()
            .filter(|t| text[pos..].starts_with(t.content.as_str()))
            .max_by_key(|t| t.content.len());
        let Some(token) = best else {
            pos += text[pos..].chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let mut start = pos;
        let mut stop = pos + token.content.len();
        if token.lstrip {
            let trimmed = text[last..start].trim_end();
            start = last + trimmed.len();
        }
        if token.rstrip {
            stop = text.len() - text[stop..].trim_start().len();
        }
        if start > last {
            out.push(Segment::Text(text[last..start].to_owned(), base + last));
        }
        out.push(Segment::Token(token.id));
        last = stop;
        pos = stop;
    }
    if last < text.len() {
        out.push(Segment::Text(text[last..].to_owned(), base + last));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tok(id: u32, content: &str, lstrip: bool, rstrip: bool) -> AddedToken {
        AddedToken {
            id,
            content: content.into(),
            normalized: false,
            lstrip,
            rstrip,
        }
    }

    #[test]
    fn longest_match_and_strip() {
        let s = tok(0, "<s>", false, false);
        let ss = tok(9, "<s>>", false, false);
        let mask = tok(4, "<mask>", true, true);
        let all = [&s, &ss, &mask];
        assert_eq!(
            split("a <s>> b  <mask>  c", 0, &all),
            vec![
                Segment::Text("a ".into(), 0),
                Segment::Token(9),
                Segment::Text(" b".into(), 6),
                Segment::Token(4),
                Segment::Text("c".into(), 18),
            ]
        );
        assert_eq!(split("<s>", 3, &all), vec![Segment::Token(0)]);
        assert_eq!(split("", 0, &all), Vec::<Segment>::new());
        assert_eq!(split("żółw", 2, &[]), vec![Segment::Text("żółw".into(), 2)]);
        assert_eq!(
            split("żółw", 2, &all),
            vec![Segment::Text("żółw".into(), 2)]
        );
    }

    #[test]
    fn parse_flags_and_rejections() {
        let parsed = parse(Some(&json!([
            {"id": 1, "content": "<pad>", "normalized": false, "lstrip": true},
            {"id": 2, "content": "x"}
        ])))
        .unwrap();
        assert_eq!(parsed[0], tok(1, "<pad>", true, false));
        assert!(parsed[1].normalized);
        assert!(parse(None).unwrap().is_empty());
        assert!(
            parse(Some(
                &json!([{"id": 1, "content": "a", "single_word": true}])
            ))
            .is_err()
        );
        assert!(parse(Some(&json!([{"content": "a"}]))).is_err());
    }
}
