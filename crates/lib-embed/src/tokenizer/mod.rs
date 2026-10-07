//! Minimalny tokenizer `tokenizer.json` (HF) dla modeli SentencePiece Unigram (XLM-R:
//! `multilingual-e5-*`, `paraphrase-multilingual-*`), zgodny z HF `tokenizers` 0.23 na wektorach
//! referencyjnych (`tests/tokenizer.rs`). Crate `tokenizers` odrzucony: `cargo deny` (RUSTSEC-2024-0436,
//! nieutrzymywany `paste`) i ciężkie zależności.
//!
//! Potok: tokeny dodane (przed normalizacją) → normalizatory → tokeny dodane (po normalizacji) →
//! pre-tokenizery → Unigram → obcięcie do `max_tokens − tokeny specjalne` → szablon (`<s> $A </s>`).

mod added;
mod normalize;
mod precompiled;
mod unigram;

use serde_json::Value;

use crate::error::EmbedError;
use added::{AddedToken, Segment};
use normalize::{Normalizer, PreTokenizer};
use unigram::Unigram;

pub use precompiled::{Precompiled, base64_decode};

/// Tokenizer tekstu.
#[derive(Debug, Clone)]
pub struct TextTokenizer {
    added: Vec<AddedToken>,
    normalizers: Vec<Normalizer>,
    pre: Vec<PreTokenizer>,
    model: Unigram,
    prefix: Vec<u32>,
    suffix: Vec<u32>,
    pad_id: u32,
}

fn bad(reason: impl Into<String>) -> EmbedError {
    EmbedError::Tokenizer(reason.into())
}

/// Identyfikatory tokenów specjalnych z szablonu `TemplateProcessing.single`.
fn template_ids(v: &Value) -> Result<(Vec<u32>, Vec<u32>), EmbedError> {
    let single = v
        .get("single")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("TemplateProcessing bez `single`"))?;
    let specials = v.get("special_tokens");
    let (mut prefix, mut suffix) = (Vec::new(), Vec::new());
    let mut seen_sequence = false;
    for piece in single {
        if piece.get("Sequence").is_some() {
            if seen_sequence {
                return Err(bad("TemplateProcessing: dwie sekwencje w `single`"));
            }
            seen_sequence = true;
            continue;
        }
        let name = piece
            .pointer("/SpecialToken/id")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("TemplateProcessing: nieznany element"))?;
        let ids = specials
            .and_then(|s| s.get(name))
            .and_then(|s| s.get("ids"))
            .and_then(Value::as_array)
            .ok_or_else(|| bad(format!("TemplateProcessing: brak `{name}`")))?;
        for id in ids {
            let id = id
                .as_u64()
                .and_then(|u| u32::try_from(u).ok())
                .ok_or_else(|| bad("TemplateProcessing: zły identyfikator"))?;
            if seen_sequence {
                suffix.push(id);
            } else {
                prefix.push(id);
            }
        }
    }
    if seen_sequence {
        Ok((prefix, suffix))
    } else {
        Err(bad("TemplateProcessing bez `$A`"))
    }
}

fn pair_id(v: &Value, key: &str) -> Result<u32, EmbedError> {
    v.get(key)
        .and_then(|p| p.get(1))
        .and_then(Value::as_u64)
        .and_then(|u| u32::try_from(u).ok())
        .ok_or_else(|| bad(format!("post_processor: brak `{key}`")))
}

fn post_processor(v: &Value) -> Result<(Vec<u32>, Vec<u32>), EmbedError> {
    if v.is_null() {
        return Ok((Vec::new(), Vec::new()));
    }
    match v.get("type").and_then(Value::as_str) {
        Some("TemplateProcessing") => template_ids(v),
        Some("RobertaProcessing" | "BertProcessing") => {
            Ok((vec![pair_id(v, "cls")?], vec![pair_id(v, "sep")?]))
        }
        Some(other) => Err(bad(format!("nieobsługiwany post_processor `{other}`"))),
        None => Err(bad("post_processor bez `type`")),
    }
}

impl TextTokenizer {
    /// Ładuje `tokenizer.json`.
    pub fn from_json(json: &str) -> Result<Self, EmbedError> {
        let v: Value = serde_json::from_str(json).map_err(|e| bad(format!("JSON: {e}")))?;
        let model = Unigram::parse(v.get("model").unwrap_or(&Value::Null))?;
        let added = added::parse(v.get("added_tokens"))?;
        let normalizers = Normalizer::parse(v.get("normalizer").unwrap_or(&Value::Null))?;
        let pre = PreTokenizer::parse(v.get("pre_tokenizer").unwrap_or(&Value::Null))?;
        let (prefix, suffix) = post_processor(v.get("post_processor").unwrap_or(&Value::Null))?;
        let vocab = model.len();
        if added
            .iter()
            .map(|t| t.id)
            .chain(prefix.iter().copied())
            .chain(suffix.iter().copied())
            .any(|id| id as usize >= vocab)
        {
            return Err(bad("identyfikator tokenu spoza słownika modelu"));
        }
        let pad_id = v
            .pointer("/padding/pad_id")
            .and_then(Value::as_u64)
            .and_then(|u| u32::try_from(u).ok())
            .or_else(|| added.iter().find(|t| t.content == "<pad>").map(|t| t.id))
            .or_else(|| model.id_of("<pad>"))
            .unwrap_or(0);
        Ok(Self {
            added,
            normalizers,
            pre,
            model,
            prefix,
            suffix,
            pad_id,
        })
    }

    /// Identyfikator wypełnienia (`<pad>`; tylko kosmetyka — maska uwagi wyklucza wypełnienie).
    pub fn pad_id(&self) -> u32 {
        self.pad_id
    }

    /// Rozmiar słownika.
    pub fn vocab_size(&self) -> usize {
        self.model.len()
    }

    /// Liczba tokenów specjalnych dodawanych przez szablon.
    pub fn special_count(&self) -> usize {
        self.prefix.len() + self.suffix.len()
    }

    fn normalize(&self, text: &str) -> String {
        self.normalizers
            .iter()
            .fold(text.to_owned(), |acc, n| n.apply(&acc))
    }

    /// Identyfikatory tekstu z tokenami specjalnymi, obcięte do `max_tokens` (z tokenami specjalnymi;
    /// obcinany jest koniec tekstu, jak `TruncationParams` w HF).
    pub fn encode(&self, text: &str, max_tokens: usize) -> Vec<u32> {
        let raw: Vec<&AddedToken> = self.added.iter().filter(|t| !t.normalized).collect();
        let norm: Vec<&AddedToken> = self.added.iter().filter(|t| t.normalized).collect();
        let mut body = Vec::new();
        for segment in added::split(text, 0, &raw) {
            let (piece, offset) = match segment {
                Segment::Token(id) => {
                    body.push(id);
                    continue;
                }
                Segment::Text(piece, offset) => (piece, offset),
            };
            let normalized = self.normalize(&piece);
            for inner in added::split(&normalized, offset, &norm) {
                match inner {
                    Segment::Token(id) => body.push(id),
                    Segment::Text(t, off) => {
                        let pieces = self
                            .pre
                            .iter()
                            .fold(vec![(t, off == 0)], |acc, p| p.apply(acc));
                        for (p, _) in pieces {
                            self.model.encode(&p, &mut body);
                        }
                    }
                }
            }
        }
        body.truncate(max_tokens.saturating_sub(self.special_count()));
        let mut ids = Vec::with_capacity(body.len() + self.special_count());
        ids.extend_from_slice(&self.prefix);
        ids.extend(body);
        ids.extend_from_slice(&self.suffix);
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tokenizer(post: Value) -> Result<TextTokenizer, EmbedError> {
        let v = json!({
            "added_tokens": [{"id": 0, "content": "<s>", "normalized": false}, {"id": 2, "content": "</s>", "normalized": false}],
            "normalizer": {"type": "Lowercase"},
            "pre_tokenizer": {"type": "Metaspace", "replacement": "▁", "prepend_scheme": "always"},
            "post_processor": post,
            "model": {"type": "Unigram", "unk_id": 1, "vocab": [["<s>", 0.0], ["<unk>", 0.0], ["</s>", 0.0], ["▁ala", -1.0], ["▁ma", -1.0], ["▁kota", -1.0], ["<pad>", 0.0]]}
        });
        TextTokenizer::from_json(&v.to_string())
    }

    #[test]
    fn roberta_and_template_processors_agree() {
        let roberta =
            tokenizer(json!({"type": "RobertaProcessing", "cls": ["<s>", 0], "sep": ["</s>", 2]}))
                .unwrap();
        let template = tokenizer(json!({
            "type": "TemplateProcessing",
            "single": [{"SpecialToken": {"id": "<s>", "type_id": 0}}, {"Sequence": {"id": "A", "type_id": 0}}, {"SpecialToken": {"id": "</s>", "type_id": 0}}],
            "special_tokens": {"<s>": {"id": "<s>", "ids": [0]}, "</s>": {"id": "</s>", "ids": [2]}}
        }))
        .unwrap();
        for t in [&roberta, &template] {
            assert_eq!(t.encode("Ala ma KOTA", 64), [0, 3, 4, 5, 2]);
            assert_eq!(t.encode("Ala ma kota", 4), [0, 3, 4, 2]);
            assert_eq!(t.encode("ala <s>", 64), [0, 3, 1, 0, 2]);
            assert_eq!(t.special_count(), 2);
            assert_eq!(t.pad_id(), 6);
            assert_eq!(t.vocab_size(), 7);
        }
        let none = tokenizer(Value::Null).unwrap();
        assert_eq!(none.encode("ala", 1), [3]);
    }

    #[test]
    fn invalid_post_processors_fail() {
        assert!(tokenizer(json!({"type": "ByteLevel"})).is_err());
        assert!(tokenizer(json!({"type": "RobertaProcessing", "cls": ["<s>", 0]})).is_err());
        assert!(
            tokenizer(json!({"type": "TemplateProcessing", "single": [], "special_tokens": {}}))
                .is_err()
        );
        assert!(
            tokenizer(json!({"type": "RobertaProcessing", "cls": ["<s>", 0], "sep": ["</s>", 99]}))
                .is_err()
        );
        assert!(TextTokenizer::from_json("{").is_err());
    }
}
