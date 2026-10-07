//! Model Unigram (SentencePiece) z `tokenizer.json`: Viterbi po prefiksach słownika, nieznane znaki
//! z karą `min_score − 10` scalane w jeden `unk` (`fuse_unk`), jak `encode_optimized` w HF.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::EmbedError;

/// Kara za nieznany znak (SentencePiece `kUnkPenalty`).
const UNK_PENALTY: f64 = 10.0;

fn bad(reason: impl Into<String>) -> EmbedError {
    EmbedError::Tokenizer(reason.into())
}

/// Słownik Unigram.
#[derive(Debug, Clone)]
pub struct Unigram {
    ids: HashMap<String, u32>,
    scores: Vec<f64>,
    unk_id: u32,
    min_score: f64,
    max_piece_bytes: usize,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    id: u32,
    score: f64,
    start: Option<usize>,
}

impl Unigram {
    /// Parsuje element `model` (`type = Unigram`, `vocab = [[kawałek, wynik], …]`, `unk_id`).
    pub fn parse(v: &Value) -> Result<Self, EmbedError> {
        match v.get("type").and_then(Value::as_str) {
            Some("Unigram") => {}
            Some(other) => {
                return Err(bad(format!(
                    "nieobsługiwany model `{other}` (tylko Unigram)"
                )));
            }
            None => return Err(bad("model bez `type`")),
        }
        if v.get("byte_fallback").and_then(Value::as_bool) == Some(true) {
            return Err(bad("Unigram z `byte_fallback` nie jest obsługiwany"));
        }
        let vocab = v
            .get("vocab")
            .and_then(Value::as_array)
            .ok_or_else(|| bad("Unigram bez `vocab`"))?;
        let mut ids = HashMap::with_capacity(vocab.len());
        let mut scores = Vec::with_capacity(vocab.len());
        for (i, item) in vocab.iter().enumerate() {
            let piece = item.get(0).and_then(Value::as_str);
            let score = item.get(1).and_then(Value::as_f64);
            let (Some(piece), Some(score)) = (piece, score) else {
                return Err(bad(format!("vocab[{i}]: oczekiwano [napis, liczba]")));
            };
            let id = u32::try_from(i).map_err(|_| bad("za duży słownik"))?;
            ids.insert(piece.to_owned(), id);
            scores.push(score);
        }
        let unk_id = v
            .get("unk_id")
            .and_then(Value::as_u64)
            .and_then(|u| u32::try_from(u).ok())
            .filter(|u| (*u as usize) < scores.len())
            .ok_or_else(|| bad("Unigram bez poprawnego `unk_id`"))?;
        let min_score = scores.iter().copied().fold(f64::INFINITY, f64::min);
        let max_piece_bytes = ids.keys().map(String::len).max().unwrap_or(1);
        Ok(Self {
            ids,
            scores,
            unk_id,
            min_score,
            max_piece_bytes,
        })
    }

    /// Rozmiar słownika.
    pub fn len(&self) -> usize {
        self.scores.len()
    }

    /// Identyfikator kawałka.
    pub fn id_of(&self, piece: &str) -> Option<u32> {
        self.ids.get(piece).copied()
    }

    /// Identyfikatory najlepszej segmentacji fragmentu (po pre-tokenizacji).
    pub fn encode(&self, text: &str, out: &mut Vec<u32>) {
        if text.is_empty() {
            return;
        }
        let size = text.len();
        let unk_score = self.min_score - UNK_PENALTY;
        let mut best = vec![
            Node {
                id: 0,
                score: 0.0,
                start: None,
            };
            size + 1
        ];
        let mut pos = 0;
        while pos < size {
            let here = best[pos].score;
            let char_len = text[pos..].chars().next().map_or(1, char::len_utf8);
            let mut has_single = false;
            let mut end = pos;
            for c in text[pos..].chars() {
                end += c.len_utf8();
                if end - pos > self.max_piece_bytes {
                    break;
                }
                let Some(&id) = self.ids.get(&text[pos..end]) else {
                    continue;
                };
                let candidate = here + self.scores[id as usize];
                let target = &mut best[end];
                if target.start.is_none() || candidate > target.score {
                    *target = Node {
                        id,
                        score: candidate,
                        start: Some(pos),
                    };
                }
                if end - pos == char_len {
                    has_single = true;
                }
            }
            if !has_single {
                let candidate = here + unk_score;
                let target = &mut best[pos + char_len];
                if target.start.is_none() || candidate > target.score {
                    *target = Node {
                        id: self.unk_id,
                        score: candidate,
                        start: Some(pos),
                    };
                }
            }
            pos += char_len;
        }
        // Odtworzenie ścieżki od końca; sąsiednie `unk` scalone w jeden.
        let mut rev: Vec<u32> = Vec::new();
        let mut end = size;
        let mut in_unk = false;
        while end > 0 {
            let node = best[end];
            let Some(start) = node.start else {
                break;
            };
            let is_unk = node.id == self.unk_id;
            if !(is_unk && in_unk) {
                rev.push(node.id);
            }
            in_unk = is_unk;
            end = start;
        }
        out.extend(rev.into_iter().rev());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model() -> Unigram {
        Unigram::parse(&json!({
            "type": "Unigram", "unk_id": 0,
            "vocab": [["<unk>", 0.0], ["▁", -2.0], ["a", -3.0], ["b", -3.0], ["ab", -1.0], ["▁ab", -1.5], ["ż", -4.0]]
        }))
        .unwrap()
    }

    fn enc(m: &Unigram, t: &str) -> Vec<u32> {
        let mut out = Vec::new();
        m.encode(t, &mut out);
        out
    }

    #[test]
    fn viterbi_prefers_best_total_score() {
        let m = model();
        assert_eq!(enc(&m, "▁ab"), [5]);
        assert_eq!(enc(&m, "abab"), [4, 4]);
        assert_eq!(enc(&m, "ża"), [6, 2]);
        assert_eq!(enc(&m, ""), Vec::<u32>::new());
        assert_eq!(m.len(), 7);
        assert_eq!(m.id_of("ab"), Some(4));
    }

    #[test]
    fn unknown_runs_are_fused() {
        let m = model();
        assert_eq!(enc(&m, "xyz"), [0]);
        assert_eq!(enc(&m, "axyb"), [2, 0, 3]);
        assert_eq!(enc(&m, "x▁y"), [0, 1, 0]);
    }

    #[test]
    fn invalid_models_are_rejected() {
        assert!(Unigram::parse(&json!({"type": "WordPiece"})).is_err());
        assert!(Unigram::parse(&json!({"vocab": []})).is_err());
        assert!(
            Unigram::parse(&json!({"type": "Unigram", "unk_id": 5, "vocab": [["a", 0.0]]}))
                .is_err()
        );
        assert!(
            Unigram::parse(&json!({"type": "Unigram", "unk_id": 0, "vocab": [["a"]]})).is_err()
        );
        assert!(
            Unigram::parse(&json!({"type": "Unigram", "unk_id": 0, "byte_fallback": true, "vocab": [["a", 0.0]]}))
                .is_err()
        );
    }
}
